use horizon_shaders::{HdrContract, Macros, Pack};
use std::{
    io::{Cursor, Write},
    path::Path,
};

#[test]
fn original_hdr_adaptation_is_explicit_and_source_bound() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/packs/compat-hdr");
    let pack = Pack::open(&path).unwrap();
    assert_eq!(HdrContract::read(&pack).unwrap().unwrap().mode, "assisted");
    let mut macros = Macros::standard();
    macros.define("HORIZON_COMPAT_HDR_OUTPUT", "1");
    let (props, _) = horizon_shaders::read_properties(&pack, &mut macros);
    let post = horizon_shaders::load_post(&pack, "", &props, &macros, &Default::default());
    assert!(
        post.compatibility.failure().is_none(),
        "{:?}",
        post.compatibility
    );
    assert!(
        post.compatibility
            .programs
            .iter()
            .any(|p| p.phase == horizon_shaders::Phase::Adaptation)
    );
    for authored in [false, true] {
        let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
        for name in pack.all_files() {
            zip.start_file(
                format!("shaders/{name}"),
                zip::write::SimpleFileOptions::default(),
            )
            .unwrap();
            let mut code = pack.read_text(&name).unwrap();
            if name == "composite.fsh" {
                code.push_str("\n// edited source\n");
            }
            if authored && name == horizon_shaders::HDR_CONTRACT {
                code = code
                    .replace("mode=assisted", "mode=authored")
                    .replace("adapter=original-linear-v1", "");
            }
            zip.write_all(code.as_bytes()).unwrap();
        }
        let modified = Pack::from_zip_bytes("changed", zip.finish().unwrap().into_inner()).unwrap();
        assert_eq!(HdrContract::read(&modified).is_ok(), authored);
    }
    let sdr =
        Pack::open(&Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/packs/compat-baseline"))
            .unwrap();
    assert!(HdrContract::read(&sdr).unwrap().is_none());
}
