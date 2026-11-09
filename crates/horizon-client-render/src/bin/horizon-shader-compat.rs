//! Inspect and validate GLSL packs through Horizon's production compatibility runtime.
use horizon_client_render::gfx::compatibility::ValidationContext;
use horizon_shaders::{Compatibility, Macros, Pack, Phase};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fs,
    path::{Component, Path, PathBuf},
    process::ExitCode,
    time::Instant,
};

const USAGE: &str = "horizon-shader-compat inspect|convert|validate <pack> [--backend vulkan|gl] [--dimension <id>] [--profile <name>] [--option KEY=VALUE] [--cache <directory>] [--out <directory>] [--hdr] [--json]\nhorizon-shader-compat report <report.json>";

struct Arguments {
    command: String,
    input: PathBuf,
    backend: String,
    dimension: String,
    profile: Option<String>,
    options: BTreeMap<String, String>,
    cache: PathBuf,
    out: Option<PathBuf>,
    json: bool,
    hdr: bool,
}

fn arguments(args: &[String]) -> Result<Arguments, String> {
    if args.len() < 2 || !["inspect", "convert", "validate", "report"].contains(&args[0].as_str()) {
        return Err(USAGE.into());
    }
    let mut out = Arguments {
        command: args[0].clone(),
        input: PathBuf::from(&args[1]),
        backend: "vulkan".into(),
        dimension: "minecraft:overworld".into(),
        profile: None,
        options: BTreeMap::new(),
        cache: std::env::temp_dir().join("horizon-shader-compat-cache"),
        out: None,
        json: false,
        hdr: false,
    };
    let mut rest = args[2..].iter();
    while let Some(flag) = rest.next() {
        if flag == "--hdr" {
            out.hdr = true;
            continue;
        }
        if flag == "--json" {
            out.json = true;
            continue;
        }
        if ![
            "--backend",
            "--dimension",
            "--profile",
            "--option",
            "--cache",
            "--out",
        ]
        .contains(&flag.as_str())
        {
            return Err(format!("unknown argument: {flag}\n{USAGE}"));
        }
        let value = rest.next().ok_or_else(|| format!("{flag} needs a value"))?;
        match flag.as_str() {
            "--backend" => out.backend = value.clone(),
            "--dimension" => out.dimension = value.clone(),
            "--profile" => out.profile = Some(value.clone()),
            "--cache" => out.cache = value.into(),
            "--out" => out.out = Some(value.into()),
            "--option" => {
                let (key, value) = value
                    .split_once('=')
                    .filter(|(k, _)| !k.is_empty())
                    .ok_or("--option needs KEY=VALUE")?;
                out.options.insert(key.into(), value.into());
            }
            _ => unreachable!(),
        }
    }
    if !["vulkan", "gl"].contains(&out.backend.as_str()) {
        return Err("backend must be vulkan or gl".into());
    }
    if out.command == "convert" && out.out.is_none() {
        return Err("convert requires --out <new directory>".into());
    }
    if out.command != "convert" && out.out.is_some() {
        return Err("--out is only valid for convert".into());
    }
    Ok(out)
}

fn legacy_dimension(name: &str) -> i32 {
    match name {
        "minecraft:the_nether" => -1,
        "minecraft:the_end" => 1,
        _ => 0,
    }
}

fn source_hash(pack: &Pack) -> Result<String, String> {
    let mut hash = Sha256::new();
    for name in pack.all_files() {
        let bytes = pack
            .read(&name)
            .ok_or_else(|| format!("cannot read pack file {name}"))?;
        hash.update((name.len() as u64).to_le_bytes());
        hash.update(name.as_bytes());
        hash.update((bytes.len() as u64).to_le_bytes());
        hash.update(bytes);
    }
    Ok(format!("{:x}", hash.finalize()))
}

fn safe_relative(name: &str) -> Result<PathBuf, String> {
    if name.contains(['\\', ':'])
        || name
            .split('/')
            .any(|part| part.is_empty() || part == "." || part == "..")
    {
        return Err(format!("unsafe pack path {name:?}"));
    }
    let path = PathBuf::from(name);
    if !path
        .components()
        .all(|part| matches!(part, Component::Normal(_)))
    {
        return Err(format!("unsafe pack path {name:?}"));
    }
    Ok(path)
}

fn output_path(input: &Path, out: &Path) -> Result<PathBuf, String> {
    if out.exists()
        || out
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("hshader"))
    {
        return Err("output must be a new directory; native .hshader export is not available for this runtime".into());
    }
    let input = fs::canonicalize(input).map_err(|e| e.to_string())?;
    let parent = out
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let parent = fs::canonicalize(parent).map_err(|e| format!("output parent: {e}"))?;
    let name = out.file_name().ok_or("output needs a directory name")?;
    let output = parent.join(name);
    let folded_input = input.to_string_lossy().to_lowercase();
    let folded_output = output.to_string_lossy().to_lowercase();
    if folded_output == folded_input
        || (input.is_dir() && Path::new(&folded_output).starts_with(Path::new(&folded_input)))
    {
        return Err("output overlaps the input pack".into());
    }
    Ok(output)
}

fn write_file(root: &Path, name: &str, bytes: &[u8]) -> Result<(), String> {
    let path = root.join(safe_relative(name)?);
    fs::create_dir_all(path.parent().unwrap()).map_err(|e| e.to_string())?;
    fs::write(path, bytes).map_err(|e| e.to_string())
}

fn check_cache_path(input: &Path, cache: &Path) -> Result<(), String> {
    if !input.is_dir() {
        return Ok(());
    }
    let input = fs::canonicalize(input).map_err(|e| e.to_string())?;
    let mut parent = std::path::absolute(cache).map_err(|e| e.to_string())?;
    let mut missing = Vec::new();
    while !parent.exists() {
        missing.push(parent.file_name().ok_or("invalid cache path")?.to_owned());
        if !parent.pop() {
            return Err("invalid cache path".into());
        }
    }
    let mut resolved = fs::canonicalize(parent).map_err(|e| e.to_string())?;
    for part in missing.into_iter().rev() {
        resolved.push(part);
    }
    let source = input.to_string_lossy().to_lowercase();
    let cache = resolved.to_string_lossy().to_lowercase();
    if Path::new(&cache).starts_with(Path::new(&source)) {
        return Err("cache must stay outside the original pack directory".into());
    }
    Ok(())
}

fn convert(args: &Arguments, pack: &Pack, macros: &Macros, report: &Value) -> Result<(), String> {
    let output = output_path(&args.input, args.out.as_ref().unwrap())?;
    let staging = output.with_file_name(format!(
        "{}.incomplete-{}",
        output.file_name().unwrap().to_string_lossy(),
        std::process::id()
    ));
    fs::create_dir(&staging).map_err(|e| e.to_string())?;
    let mut generated_hashes = BTreeMap::new();
    let mut sources = std::collections::BTreeSet::new();
    for name in pack.all_files().into_iter().filter(|name| {
        ["vsh", "fsh", "gsh", "csh", "tcs", "tes"]
            .contains(&name.rsplit('.').next().unwrap_or_default())
    }) {
        sources.extend(horizon_shaders::preprocess(pack, &name, macros).files);
    }
    for name in pack.all_files() {
        let textual = sources.contains(&name)
            || ["vsh", "fsh", "gsh", "csh", "glsl", "inc", "tcs", "tes"]
                .contains(&name.rsplit('.').next().unwrap_or_default());
        let bytes = if textual {
            pack.read_text(&name).map(String::into_bytes)
        } else {
            pack.read(&name)
        }
        .ok_or_else(|| format!("cannot read {name}"))?;
        write_file(&staging, &format!("shaders/{name}"), &bytes)?;
        generated_hashes.insert(name.clone(), format!("{:x}", Sha256::digest(&bytes)));
        if ["vsh", "fsh", "gsh", "csh"].contains(&name.rsplit('.').next().unwrap_or_default()) {
            let pp = horizon_shaders::preprocess(pack, &name, macros);
            let bytes = serde_json::to_vec_pretty(&pp).map_err(|e| e.to_string())?;
            write_file(&staging, &format!("generated/{name}.json"), &bytes)?;
        }
    }
    preserve_notices(&args.input, &staging)?;
    if args.hdr {
        write_file(
            &staging,
            "generated/horizon-linear-scene-v1.json",
            &serde_json::to_vec_pretty(&horizon_shaders::HdrContract::fragment())
                .map_err(|e| e.to_string())?,
        )?;
    }
    write_file(
        &staging,
        "compatibility-report.json",
        &serde_json::to_vec_pretty(report).map_err(|e| e.to_string())?,
    )?;
    write_file(&staging, "SOURCE.json", &serde_json::to_vec_pretty(&json!({"original":fs::canonicalize(&args.input).map_err(|e|e.to_string())?,"source_sha256":report["source_sha256"],"generated_sha256":generated_hashes,"settings_baked":args.options,"runtime":"Horizon GLSL compatibility","native_export":false})).map_err(|e|e.to_string())?)?;
    if source_hash(pack)? != report["source_sha256"].as_str().unwrap_or_default() {
        return Err("input changed during conversion; discard the incomplete output and retry with stable files".into());
    }
    fs::rename(&staging, output).map_err(|e| e.to_string())
}

fn preserve_notices(input: &Path, output: &Path) -> Result<(), String> {
    use std::io::Read;
    let notice = |name: &str| {
        let upper = name.to_uppercase();
        ["LICENSE", "LICENCE", "COPYING", "NOTICE"]
            .iter()
            .any(|prefix| upper.starts_with(prefix))
    };
    if input.is_dir() {
        let mut directories = vec![PathBuf::new()];
        while let Some(directory) = directories.pop() {
            for entry in fs::read_dir(input.join(&directory)).map_err(|e| e.to_string())? {
                let entry = entry.map_err(|e| e.to_string())?;
                let name = entry.file_name().to_string_lossy().into_owned();
                if directory.as_os_str().is_empty() && !notice(&name) {
                    continue;
                }
                let relative = directory.join(&name);
                let kind = entry.file_type().map_err(|e| e.to_string())?;
                if kind.is_dir() {
                    directories.push(relative);
                } else if kind.is_file() {
                    let mut bytes = Vec::new();
                    fs::File::open(entry.path())
                        .map_err(|e| e.to_string())?
                        .take((1 << 20) + 1)
                        .read_to_end(&mut bytes)
                        .map_err(|e| e.to_string())?;
                    if bytes.len() > 1 << 20 {
                        return Err("notice file is larger than 1 MiB".into());
                    }
                    write_file(
                        output,
                        &relative.to_string_lossy().replace('\\', "/"),
                        &bytes,
                    )?;
                }
            }
        }
    } else {
        let mut zip = zip::ZipArchive::new(fs::File::open(input).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
        for index in 0..zip.len() {
            let mut entry = zip.by_index(index).map_err(|e| e.to_string())?;
            let name = entry.name().map_err(|e| e.to_string())?.into_owned();
            if notice(name.split('/').next().unwrap_or_default()) && entry.is_file() {
                let mut bytes = Vec::new();
                entry
                    .by_ref()
                    .take((1 << 20) + 1)
                    .read_to_end(&mut bytes)
                    .map_err(|e| e.to_string())?;
                if bytes.len() > 1 << 20 {
                    return Err("root notice is larger than 1 MiB".into());
                }
                write_file(output, &name, &bytes)?;
            }
        }
    }
    Ok(())
}

fn human(report: &Value) {
    println!(
        "{}: {}",
        report["pack"].as_str().unwrap_or("pack"),
        report["compatibility"].as_str().unwrap_or("untested")
    );
    println!("evidence: {}", report["evidence"]);
    if let Some(programs) = report["outcomes"]["programs"].as_array() {
        for p in programs.iter().filter(|p| p["outcome"] != "loaded") {
            println!(
                "{}: {} ({}) {}",
                p["requested"], p["outcome"], p["phase"], p["message"]
            );
        }
    }
    println!("GPU execution and visual appearance require a separate scene run.");
}

fn run(mut args: Arguments) -> Result<u8, String> {
    if args.command == "report" {
        let report: Value =
            serde_json::from_slice(&fs::read(&args.input).map_err(|e| e.to_string())?)
                .map_err(|e| e.to_string())?;
        if report["schema"] != 1 {
            return Err("unsupported report schema".into());
        }
        if args.json {
            println!("{}", serde_json::to_string_pretty(&report).unwrap());
        } else {
            human(&report);
        }
        return Ok(0);
    }
    if let Some(output) = &args.out {
        output_path(&args.input, output)?;
    }
    check_cache_path(&args.input, &args.cache)?;
    let original = Pack::open(&args.input).map_err(|e| e.to_string())?;
    let source_hash = source_hash(&original)?;
    let mut unavailable = None;
    let mut context = if args.command == "inspect" {
        None
    } else {
        match ValidationContext::open(if args.backend == "vulkan" {
            wgpu::Backends::VULKAN
        } else {
            wgpu::Backends::GL
        }) {
            Ok(context) => Some(context),
            Err(reason) => {
                unavailable = Some(reason);
                None
            }
        }
    };
    let available = horizon_shaders::options(&original);
    let mut defaults = Macros::standard();
    horizon_client_render::gfx::compatibility::define_macros(
        &mut defaults,
        4,
        context
            .as_ref()
            .map_or(wgpu::Features::empty(), |c| c.device.features()),
        args.hdr,
    );
    let (default_properties, _) = horizon_shaders::read_properties(&original, &mut defaults);
    let profiles = horizon_shaders::Screens::parse(&default_properties, &available).profiles;
    if let Some(profile) = &args.profile {
        let values = profiles
            .iter()
            .find(|(name, _)| name == profile)
            .ok_or_else(|| format!("unknown profile {profile}"))?;
        let mut selected: BTreeMap<_, _> = values.1.iter().cloned().collect();
        selected.extend(args.options);
        args.options = selected;
    }
    for (key, value) in &args.options {
        let option = available
            .get(key)
            .ok_or_else(|| format!("unknown pack option {key}"))?;
        if !option.values.contains(value) {
            return Err(format!("invalid value for option {key}"));
        }
    }
    let identity = context
        .as_ref()
        .map_or_else(|| "inspection".into(), ValidationContext::identity);
    let pack = original.with_chosen(args.options.clone()).with_cache(
        args.cache.clone(),
        format!("{identity};dimension={}", args.dimension),
    );
    let files = pack.all_files();
    let mut dimensions: std::collections::BTreeSet<String> = [String::new()].into();
    for file in &files {
        if let Some((folder, _)) = file.split_once('/')
            && folder.starts_with("world")
        {
            dimensions.insert(folder.into());
        }
    }
    if let Some(text) = pack.read_text("dimension.properties") {
        for entry in horizon_shaders::Properties::parse(&text, &Macros::default()).entries {
            if let Some(folder) = entry.key.strip_prefix("dimension.") {
                dimensions.insert(folder.into());
            }
        }
    }
    let mut macros = defaults;
    let (properties, _) = horizon_shaders::read_properties(&pack, &mut macros);
    let folder = horizon_shaders::dimension_folder_named(
        &pack,
        &args.dimension,
        legacy_dimension(&args.dimension),
    );
    let mut report = json!({
        "schema":1,"upstream_base":"42d1e6d7d77ed300fcc4d644ce1444c25356085c","compiler_id":horizon_shaders::COMPILER_ID,
        "pack":pack.name(),"source_sha256":source_hash,"dimension":args.dimension,"folder":folder,"profile":args.profile,"options":args.options,"requested_backend":args.backend,"hdr_handoff":args.hdr,
        "validation_scope":"active programs for the selected dimension and effective options",
        "inventory":{"files":files,"options":available.values().map(|o|json!({"name":o.name,"default":o.default,"values":o.values})).collect::<Vec<_>>(),"profiles":properties.entries.iter().filter(|e|e.key.starts_with("profile.")).map(|e|&e.key).collect::<Vec<_>>(),"required_features":properties.get("iris.features.required")},
        "compatibility":"untested","outcomes":Compatibility::default(),
        "evidence":{"preprocessing":"not_run","translation":"not_run","backend_compilation":"not_run","gpu_execution":"not_run","visual_validation":"not_run"},
        "transformations":[],"semantic_changes":[],
        "native_export":{"available":false,"reason":"native stages have not been proven equivalent for this pack"},
        "limitations":["Pipeline validation uses generated resources, not a game scene or visual reference.","Generated output uses the compatibility runtime. Original files remain at the recorded source path."]
    });
    report["inventory"]["dimensions"] = json!(dimensions);
    report["inventory"]["stages"] = json!(
        files
            .iter()
            .filter(|file| ["vsh", "fsh", "gsh", "csh", "tcs", "tes"]
                .contains(&file.rsplit('.').next().unwrap_or_default()))
            .collect::<Vec<_>>()
    );
    let mut passed = true;
    if let Some(reason) = &unavailable {
        passed = false;
        report["reason"] = json!(reason);
        report["evidence"]["backend_compilation"] = json!("unavailable");
    }
    if let Some(context) = &mut context {
        let started = Instant::now();
        let inputs = context.inputs(
            properties
                .get("separateAo")
                .is_some_and(|v| v.trim() == "true"),
        );
        let post = horizon_shaders::load_post(&pack, &folder, &properties, &macros, &inputs);
        report["hdr_contract"] = json!(post.hdr);
        if args.hdr && post.hdr.is_none() && !pack.exists(horizon_shaders::HDR_CONTRACT) {
            return Err("--hdr requires an explicit HDR handoff contract".into());
        }
        let preprocessing_failed = post.compatibility.programs.iter().any(|p| {
            p.phase == Phase::Preprocessing && p.outcome == horizon_shaders::Outcome::Failed
        });
        let loader_failed = post.compatibility.failure().is_some();
        let translation_failed = post.compatibility.programs.iter().any(|p| {
            p.phase == Phase::Translation && p.outcome == horizon_shaders::Outcome::Failed
        });
        let built = context.validate(post);
        report["evidence"]["capability"] = json!(if built
            .compatibility
            .programs
            .iter()
            .any(|p| p.phase == Phase::Capability && p.outcome == horizon_shaders::Outcome::Failed)
        {
            "failed"
        } else if loader_failed {
            "not_run"
        } else {
            "passed"
        });
        report["semantic_changes"] = json!(
            built
                .compatibility
                .programs
                .iter()
                .filter(|p| p.phase == Phase::Adaptation
                    && p.outcome == horizon_shaders::Outcome::Loaded)
                .collect::<Vec<_>>()
        );
        report["transformations"] = json!(built.compatibility.programs.iter().filter(|p|p.phase==Phase::Translation && p.outcome==horizon_shaders::Outcome::Loaded).map(|p|json!({"program":p.requested,"operation":"production GLSL lowering and Naga semantic validation","source_map":"original preprocessor file and line map retained"})).collect::<Vec<_>>());
        passed = built.reason.is_none();
        report["outcomes"] = serde_json::to_value(&built.compatibility).unwrap();
        report["reason"] = json!(built.reason);
        report["notes"] = json!(built.notes);
        report["compatibility"] = json!(if passed { "untested" } else { "incomplete" });
        report["evidence"]["preprocessing"] = json!(if preprocessing_failed {
            "failed"
        } else {
            "passed"
        });
        report["evidence"]["translation"] = json!(if preprocessing_failed {
            "not_run"
        } else if translation_failed {
            "failed"
        } else {
            "passed"
        });
        report["evidence"]["backend_compilation"] = json!(if loader_failed {
            "not_run"
        } else if passed {
            "passed"
        } else {
            "failed"
        });
        let info = context.adapter.get_info();
        let limits = context.device.limits();
        report["adapter"] = json!({"name":info.name,"backend":format!("{:?}",info.backend),"vendor":info.vendor,"device":info.device,"driver":info.driver,"driver_info":info.driver_info});
        report["capabilities"] = json!({"enabled_features":format!("{:?}",context.device.features()),"downlevel":format!("{:?}",context.adapter.get_downlevel_capabilities().flags),"max_color_attachments":limits.max_color_attachments,"max_sampled_textures_per_stage":limits.max_sampled_textures_per_shader_stage,"max_storage_buffers_per_stage":limits.max_storage_buffers_per_shader_stage});
        report["capabilities"]["formats"] = json!([wgpu::TextureFormat::Rgba8Unorm,wgpu::TextureFormat::Rgba16Float,wgpu::TextureFormat::Rgba32Float,wgpu::TextureFormat::Rg11b10Ufloat,wgpu::TextureFormat::R32Uint,wgpu::TextureFormat::Depth32Float].map(|format| {
            let features=context.adapter.get_texture_format_features(format);
            json!({"format":format!("{format:?}"),"allowed_usages":format!("{:?}",features.allowed_usages),"flags":format!("{:?}",features.flags)})
        }));
        report["compilation_ms"] = json!(started.elapsed().as_secs_f64() * 1000.0);
    }
    if args.command == "convert" && passed {
        convert(&args, &pack, &macros, &report)?;
    }
    if args.json {
        println!("{}", serde_json::to_string_pretty(&report).unwrap());
    } else {
        human(&report);
    }
    Ok(if unavailable.is_some() {
        2
    } else if passed {
        0
    } else {
        1
    })
}

fn main() -> ExitCode {
    match arguments(&std::env::args().skip(1).collect::<Vec<_>>()).and_then(run) {
        Ok(code) => ExitCode::from(code),
        Err(error) => {
            eprintln!("{error}");
            ExitCode::from(2)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn option_baking_reaches_includes_with_arbitrary_extensions() {
        use std::io::{Cursor, Write};
        let root = std::env::temp_dir().join(format!("compat-bake-{}", std::process::id()));
        fs::create_dir(&root).unwrap();
        let input = root.join("input.zip");
        let output = root.join("converted");
        let mut archive = zip::ZipWriter::new(Cursor::new(Vec::new()));
        for (name, code) in [
            (
                "final.fsh",
                "#version 120\n#include \"shared.h\"\nvoid main(){gl_FragColor=vec4(float(GAIN));}",
            ),
            ("shared.h", "#define GAIN 1 // [1 2]\n"),
        ] {
            archive
                .start_file(
                    format!("shaders/{name}"),
                    zip::write::SimpleFileOptions::default(),
                )
                .unwrap();
            archive.write_all(code.as_bytes()).unwrap();
        }
        let bytes = archive.finish().unwrap().into_inner();
        fs::write(&input, &bytes).unwrap();
        let args = arguments(&[
            "convert".into(),
            input.to_string_lossy().into_owned(),
            "--out".into(),
            output.to_string_lossy().into_owned(),
            "--option".into(),
            "GAIN=2".into(),
        ])
        .unwrap();
        let pack = Pack::open(&input)
            .unwrap()
            .with_chosen(args.options.clone());
        convert(
            &args,
            &pack,
            &Macros::standard(),
            &json!({"source_sha256":source_hash(&pack).unwrap()}),
        )
        .unwrap();
        assert!(
            fs::read_to_string(output.join("shaders/shared.h"))
                .unwrap()
                .contains("#define GAIN 2")
        );
        assert_eq!(fs::read(&input).unwrap(), bytes);
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn nested_notices_are_preserved_without_unrelated_files() {
        let root = std::env::temp_dir().join(format!("compat-notices-{}", std::process::id()));
        fs::create_dir(&root).unwrap();
        let input = root.join("input");
        let output = root.join("output");
        fs::create_dir_all(input.join("LICENSES/nested")).unwrap();
        fs::create_dir(&output).unwrap();
        fs::write(
            input.join("LICENSES/nested/component.txt"),
            "Original notice",
        )
        .unwrap();
        fs::write(input.join("unrelated.txt"), "Not a notice").unwrap();
        assert!(check_cache_path(&input, &input.join("shaders/cache/new")).is_err());
        assert!(check_cache_path(&input, &input.join("LICENSES")).is_err());
        assert!(check_cache_path(&input, &root.join("outside-cache")).is_ok());
        preserve_notices(&input, &output).unwrap();
        assert_eq!(
            fs::read_to_string(output.join("LICENSES/nested/component.txt")).unwrap(),
            "Original notice"
        );
        assert!(!output.join("unrelated.txt").exists());
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn unsafe_export_paths_and_unknown_arguments_are_rejected() {
        for path in [
            "../escape",
            "a/../../escape",
            "/escape",
            "C:/escape",
            "a\\escape",
            "a//b",
        ] {
            assert!(safe_relative(path).is_err(), "{path}");
        }
        assert_eq!(
            safe_relative("world0/composite.fsh").unwrap(),
            PathBuf::from("world0/composite.fsh")
        );
        for args in [
            vec!["validate", "pack", "--backend", "invalid"],
            vec!["convert", "pack"],
            vec!["inspect", "pack", "--unknown"],
        ] {
            assert!(arguments(&args.into_iter().map(str::to_owned).collect::<Vec<_>>()).is_err());
        }
    }
}
