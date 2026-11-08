//! Cache hits must preserve authored diagnostics and invalidate on every preprocessing input.
use horizon_shaders::{Macros, Pack, preprocess};
use std::{
    fs,
    time::{SystemTime, UNIX_EPOCH},
};

#[test]
fn cached_stages_track_includes_options_macros_context_and_corruption() {
    let root = std::env::temp_dir().join(format!(
        "horizon-original-cache-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let source = root.join("source");
    let shaders = source.join("shaders");
    let cache = root.join("cache");
    fs::create_dir_all(&shaders).unwrap();
    fs::write(
        shaders.join("composite.fsh"),
        "#version 120\n#include \"value.glsl\"\nvoid main(){gl_FragColor=vec4(VALUE+EXTERNAL);}",
    )
    .unwrap();
    fs::write(shaders.join("value.glsl"), "#define VALUE 1 // [1 2]").unwrap();
    let pack = Pack::open(&source)
        .unwrap()
        .with_cache(cache.clone(), "vulkan/device-a".into());
    let mut macros = Macros::default();
    macros.define("EXTERNAL", "4");
    let first = preprocess(&pack, "composite.fsh", &macros);
    assert!(first.code.contains("vec4(1+4)"));
    let files: Vec<_> = fs::read_dir(&cache)
        .unwrap()
        .map(|f| f.unwrap().path())
        .collect();
    assert_eq!(files.len(), 1);
    let hit = preprocess(&pack, "composite.fsh", &macros);
    assert_eq!(hit.code, first.code);
    assert_eq!(hit.lines, first.lines);
    assert_eq!(hit.comments, first.comments);
    fs::write(&files[0], "{broken cache").unwrap();
    assert_eq!(preprocess(&pack, "composite.fsh", &macros).code, first.code);
    assert!(
        fs::read_to_string(&files[0])
            .unwrap()
            .starts_with("{\"dependencies\":")
    );
    fs::write(shaders.join("value.glsl"), "#define VALUE 2 // [1 2]").unwrap();
    assert!(
        preprocess(&pack, "composite.fsh", &macros)
            .code
            .contains("vec4(2+4)")
    );
    macros.define("EXTERNAL", "8");
    assert!(
        preprocess(&pack, "composite.fsh", &macros)
            .code
            .contains("vec4(2+8)")
    );
    let selected = Pack::open(&source)
        .unwrap()
        .with_chosen([("VALUE".into(), "1".into())].into())
        .with_cache(cache.clone(), "vulkan/device-a".into());
    assert!(
        preprocess(&selected, "composite.fsh", &macros)
            .code
            .contains("vec4(1+8)")
    );
    let other = Pack::open(&source)
        .unwrap()
        .with_cache(cache.clone(), "gl/device-b".into());
    assert!(
        preprocess(&other, "composite.fsh", &macros)
            .code
            .contains("vec4(2+8)")
    );
    assert!(fs::read_dir(&cache).unwrap().count() >= 3);
    fs::write(shaders.join("value.glsl"), "#include \"missing.glsl\"").unwrap();
    let broken = preprocess(&other, "composite.fsh", &macros);
    assert!(!broken.diagnostics.is_empty());
    fs::write(shaders.join("missing.glsl"), "#define VALUE 1").unwrap();
    assert!(
        preprocess(&other, "composite.fsh", &macros)
            .diagnostics
            .is_empty()
    );
    fs::create_dir(shaders.join("world0")).unwrap();
    fs::write(
        shaders.join("world0/composite.fsh"),
        "void main(){gl_FragColor=vec4(0.125);}",
    )
    .unwrap();
    assert!(
        preprocess(&other, "world0/composite.fsh", &macros)
            .code
            .contains("0.125")
    );
}
