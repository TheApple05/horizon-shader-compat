//! Original controls for observable production-loader outcomes.
use horizon_shaders::{GameInputs, GbuffersInputs, Macros, Outcome, Pack, Phase, Post};
use std::io::{Cursor, Write};

const VERTEX: &str = "#version 120\nvoid main(){gl_Position=ftransform();}\n";
const FRAGMENT: &str = "#version 120\nvoid main(){gl_FragColor=vec4(0.25);}\n";

fn load(files: &[(&str, &str)], inputs: GameInputs) -> Post {
    let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for (name, contents) in files {
        zip.start_file(
            format!("shaders/{name}"),
            zip::write::SimpleFileOptions::default(),
        )
        .unwrap();
        zip.write_all(contents.as_bytes()).unwrap();
    }
    let pack = Pack::from_zip_bytes("original-report", zip.finish().unwrap().into_inner()).unwrap();
    let mut macros = Macros::standard();
    let (properties, _) = horizon_shaders::read_properties(&pack, &mut macros);
    horizon_shaders::load_post(&pack, "", &properties, &macros, &inputs)
}

#[test]
fn errors_keep_the_original_include_location() {
    let post = load(
        &[
            ("composite.vsh", VERTEX),
            (
                "composite.fsh",
                "#version 120\n#include \"absent.glsl\"\nvoid main(){}",
            ),
        ],
        GameInputs::default(),
    );
    let failure = post.compatibility.failure().unwrap();
    assert_eq!(failure.phase, Phase::Preprocessing);
    assert_eq!(failure.file.as_deref(), Some("composite.fsh"));
    assert_eq!(failure.line, Some(2));
    assert_eq!(failure.requested, "composite");
}

#[test]
fn enabled_incomplete_stages_fail_but_disabled_broken_stages_do_not() {
    let missing = load(&[("composite.vsh", VERTEX)], GameInputs::default());
    assert_eq!(
        missing.compatibility.failure().unwrap().phase,
        Phase::Selection
    );
    let disabled = load(
        &[
            ("composite.vsh", VERTEX),
            ("composite.fsh", "bad syntax"),
            ("shaders.properties", "program.composite.enabled=false"),
        ],
        GameInputs::default(),
    );
    assert!(disabled.compatibility.failure().is_none());
    assert!(
        disabled
            .compatibility
            .programs
            .iter()
            .any(|p| p.requested == "composite" && p.outcome == Outcome::Disabled)
    );
}

#[test]
fn compute_and_required_features_cannot_disappear_from_a_success_report() {
    let post = load(
        &[(
            "composite.csh",
            "#version 430\nlayout(local_size_x=1) in;void main(){nonexistent();}",
        )],
        GameInputs::default(),
    );
    let failure = post.compatibility.failure().unwrap();
    assert_eq!(failure.requested, "composite");
    assert_eq!(failure.phase, Phase::Translation);
    assert_eq!(failure.file.as_deref(), Some("composite.csh"));
    let post = load(
        &[(
            "shaders.properties",
            "iris.features.required=ORIGINAL_UNAVAILABLE_FEATURE",
        )],
        GameInputs::default(),
    );
    assert_eq!(
        post.compatibility.failure().unwrap().capability.as_deref(),
        Some("ORIGINAL_UNAVAILABLE_FEATURE")
    );
}

#[test]
fn documented_fallback_resolves_only_its_own_selection_attempt() {
    let files = [
        ("gbuffers_terrain.vsh", VERTEX),
        ("gbuffers_terrain.fsh", "bad syntax"),
        ("gbuffers_basic.vsh", VERTEX),
        ("gbuffers_basic.fsh", FRAGMENT),
    ];
    let post = load(
        &files,
        GameInputs {
            terrain: Some(GbuffersInputs::default()),
            ..Default::default()
        },
    );
    assert_eq!(post.terrain.as_ref().unwrap().name, "gbuffers_basic");
    assert!(post.compatibility.failure().is_none());
    assert!(
        post.compatibility
            .programs
            .iter()
            .any(|p| p.requested == "gbuffers_terrain"
                && p.outcome == Outcome::Fallback
                && p.selected.as_deref() == Some("gbuffers_basic"))
    );
    let post = load(
        &[
            ("composite.vsh", VERTEX),
            ("composite.fsh", "bad syntax"),
            files[0],
            files[1],
            files[2],
            files[3],
        ],
        GameInputs {
            terrain: Some(GbuffersInputs::default()),
            ..Default::default()
        },
    );
    assert_eq!(post.compatibility.failure().unwrap().requested, "composite");
}

#[test]
fn optional_material_variants_keep_the_legacy_path_on_rejection() {
    let post = load(
        &[
            ("gbuffers_block_translucent.vsh", VERTEX),
            ("gbuffers_block_translucent.fsh", "bad syntax"),
        ],
        GameInputs {
            entities: Some(GbuffersInputs::default()),
            ..Default::default()
        },
    );
    assert!(post.compatibility.failure().is_none());
    assert!(
        post.compatibility
            .programs
            .iter()
            .any(|p| p.requested == "gbuffers_block_translucent" && p.outcome == Outcome::Fallback)
    );
}
