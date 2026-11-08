//! Explicit linear scene handoff; no HDR inference from an arbitrary SDR final image.
use crate::{Macros, Pack, Preprocessed, Properties};
use sha2::{Digest, Sha256};

pub const HDR_CONTRACT: &str = "horizon-hdr.properties";
const ORIGINAL: &str = "c6f00eedd8d14e876b282b37be57fdf59143701c155398b29adbcb649a1d089b";

#[derive(Clone, Debug, serde::Serialize)]
pub struct HdrContract {
    pub mode: String,
}

impl HdrContract {
    pub fn read(pack: &Pack) -> Result<Option<Self>, String> {
        if !pack.exists(HDR_CONTRACT) {
            return Ok(None);
        }
        let text = pack
            .read_text(HDR_CONTRACT)
            .ok_or_else(|| format!("cannot read {HDR_CONTRACT}"))?;
        let props = Properties::parse(&text, &Macros::default());
        let distinct: std::collections::BTreeSet<_> =
            props.entries.iter().map(|e| &e.key).collect();
        for (key, value) in [
            ("contract", "1"),
            ("scene", "colortex0"),
            ("encoding", "linear-bt709"),
            ("white", "paper-white"),
            ("exposure", "pack"),
            ("tonemap", "horizon"),
        ] {
            if props.get(key) != Some(value) {
                return Err(format!("{HDR_CONTRACT}: requires {key}={value}"));
            }
        }
        if !props.notes.is_empty()
            || distinct.len() != props.entries.len()
            || props.entries.iter().any(|e| {
                ![
                    "contract", "scene", "encoding", "white", "exposure", "tonemap", "mode",
                    "adapter",
                ]
                .contains(&e.key.as_str())
            })
        {
            return Err(format!("{HDR_CONTRACT}: invalid or unknown declaration"));
        }
        let mode = props.get("mode").unwrap_or_default();
        match mode {
            "authored" if props.get("adapter").is_none() => {}
            "assisted"
                if props.get("adapter") == Some("original-linear-v1")
                    && Self::source_hash(pack)? == ORIGINAL => {}
            _ => {
                return Err(format!(
                    "{HDR_CONTRACT}: unknown contract or unverified assisted source; use the unchanged original HDR fixture"
                ));
            }
        }
        Ok(Some(Self { mode: mode.into() }))
    }

    /// Stable across checkout line endings; raw archive hashes remain separate evidence.
    pub fn source_hash(pack: &Pack) -> Result<String, String> {
        let mut hash = Sha256::new();
        for name in pack.all_files().into_iter().filter(|n| n != HDR_CONTRACT) {
            let bytes = pack
                .read(&name)
                .ok_or_else(|| format!("cannot read {name}"))?;
            let bytes = match std::str::from_utf8(&bytes) {
                Ok(text) => text
                    .trim_start_matches('\u{feff}')
                    .replace("\r\n", "\n")
                    .into_bytes(),
                Err(_) => bytes,
            };
            hash.update((name.len() as u64).to_le_bytes());
            hash.update(name.as_bytes());
            hash.update((bytes.len() as u64).to_le_bytes());
            hash.update(bytes);
        }
        Ok(format!("{:x}", hash.finalize()))
    }

    pub fn fragment() -> Preprocessed {
        let code = "uniform sampler2D colortex0;\nuniform float viewWidth;\nuniform float viewHeight;\nvoid main(){\nvec4 scene=texture2D(colortex0,gl_FragCoord.xy/vec2(viewWidth,viewHeight));\ngl_FragColor=vec4(pow(max(scene.rgb,vec3(0.0)),vec3(1.0/2.2)),scene.a);\n}\n";
        Preprocessed {
            code: code.into(),
            files: vec!["<horizon-linear-scene-v1>".into()],
            lines: (1..=code.lines().count())
                .map(|line| crate::Origin {
                    file: 0,
                    line: line as u32,
                })
                .collect(),
            ..Default::default()
        }
    }
}
