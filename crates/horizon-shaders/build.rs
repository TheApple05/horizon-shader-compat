use sha2::{Digest, Sha256};
use std::{fs, path::Path};

fn hash(path: &Path, digest: &mut Sha256) {
    println!("cargo:rerun-if-changed={}", path.display());
    if path.is_dir() {
        let mut children: Vec<_> = fs::read_dir(path)
            .unwrap()
            .map(|p| p.unwrap().path())
            .collect();
        children.sort();
        for child in children {
            hash(&child, digest);
        }
    } else {
        digest.update(path.file_name().unwrap().to_string_lossy().as_bytes());
        digest.update(fs::read(path).unwrap());
    }
}

fn main() {
    let manifest = std::path::PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").unwrap());
    let root = manifest.parent().unwrap().parent().unwrap();
    let mut digest = Sha256::new();
    for path in [
        manifest.join("src"),
        root.join("third_party/naga/src"),
        root.join("third_party/naga/Cargo.toml"),
        root.join("Cargo.lock"),
    ] {
        hash(&path, &mut digest);
    }
    println!(
        "cargo:rustc-env=HORIZON_SHADER_COMPILER_ID={:x}",
        digest.finalize()
    );
}
