//! A dependency-checked cache of original preprocessor results, including source locations.
use crate::{Macros, Pack, Preprocessed};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::Read,
    sync::atomic::{AtomicU64, Ordering},
};

const MAX_CACHE: u64 = 16 << 20;
static NEXT: AtomicU64 = AtomicU64::new(0);

#[derive(Serialize, Deserialize)]
struct Cached {
    dependencies: Vec<(String, String)>,
    stage: Preprocessed,
}

fn digest(text: &str) -> String {
    format!("{:x}", Sha256::digest(text.as_bytes()))
}

pub(crate) fn preprocess(pack: &Pack, path: &str, macros: &Macros) -> Preprocessed {
    let Some((directory, context)) = &pack.cache else {
        return crate::preprocess::uncached(pack, path, macros);
    };
    let key = digest(&format!(
        "{}\0{context}\0{path}\0{macros:?}",
        crate::COMPILER_ID
    ));
    let file = directory.join(format!("{key}.json"));
    let cached =
        (|| {
            let mut bytes = Vec::new();
            fs::File::open(&file)
                .ok()?
                .take(MAX_CACHE + 1)
                .read_to_end(&mut bytes)
                .ok()?;
            if bytes.len() as u64 > MAX_CACHE {
                return None;
            }
            let cached: Cached = serde_json::from_slice(&bytes).ok()?;
            if cached.stage.files.first().map(String::as_str) != Some(path)
                || cached.dependencies.len() != cached.stage.files.len()
                || cached.dependencies.iter().zip(&cached.stage.files).any(
                    |((name, hash), file)| {
                        name != file
                            || pack
                                .read_text(name)
                                .is_none_or(|text| digest(&text) != *hash)
                    },
                )
                || cached
                    .stage
                    .lines
                    .iter()
                    .chain(cached.stage.comments.iter().map(|c| &c.0))
                    .any(|o| o.file as usize >= cached.stage.files.len() || o.line == 0)
                || !cached.stage.diagnostics.is_empty()
            {
                return None;
            }
            Some(cached.stage)
        })();
    if let Some(stage) = cached {
        return stage;
    }
    let stage = crate::preprocess::uncached(pack, path, macros);
    if stage.diagnostics.is_empty() {
        let dependencies: Option<Vec<_>> = stage
            .files
            .iter()
            .map(|name| Some((name.clone(), digest(&pack.read_text(name)?))))
            .collect();
        if let Some(dependencies) = dependencies {
            let cached = Cached {
                dependencies,
                stage: stage.clone(),
            };
            if let Ok(bytes) = serde_json::to_vec(&cached)
                && bytes.len() as u64 <= MAX_CACHE
                && fs::create_dir_all(directory).is_ok()
            {
                let temporary = directory.join(format!(
                    "{key}.{}.{}.tmp",
                    std::process::id(),
                    NEXT.fetch_add(1, Ordering::Relaxed)
                ));
                if fs::write(&temporary, bytes).is_ok() {
                    let _ = fs::rename(&temporary, &file);
                    let _ = fs::remove_file(temporary);
                }
            }
        }
    }
    stage
}
