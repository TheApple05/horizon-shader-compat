//! Structured production-loader outcomes, independent of a GPU or presentation format.
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Phase {
    Selection,
    Preprocessing,
    Translation,
    Capability,
    BackendCompilation,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Outcome {
    Loaded,
    Disabled,
    Failed,
    Fallback,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProgramOutcome {
    pub requested: String,
    pub selected: Option<String>,
    pub phase: Phase,
    pub outcome: Outcome,
    pub file: Option<String>,
    pub line: Option<u32>,
    pub message: String,
    pub capability: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Compatibility {
    pub programs: Vec<ProgramOutcome>,
}

impl Compatibility {
    pub fn record(
        &mut self,
        name: &str,
        phase: Phase,
        outcome: Outcome,
        message: impl Into<String>,
    ) {
        // These optional variants explicitly retain the ordinary material path when rejected.
        let optional = outcome == Outcome::Failed
            && matches!(
                name,
                "gbuffers_block_translucent" | "gbuffers_particles_translucent"
            );
        let outcome = if optional { Outcome::Fallback } else { outcome };
        self.programs.push(ProgramOutcome {
            requested: name.into(),
            selected: if optional {
                Some("legacy-material-grouping".into())
            } else {
                (outcome == Outcome::Loaded).then(|| name.into())
            },
            phase,
            outcome,
            file: None,
            line: None,
            message: message.into(),
            capability: None,
        });
    }

    pub fn failure(&self) -> Option<&ProgramOutcome> {
        self.programs.iter().find(|p| p.outcome == Outcome::Failed)
    }

    /// Only this selection attempt's failures are satisfied by its documented fallback.
    pub(crate) fn resolve(&mut self, start: usize, requested: &str, selected: Option<&str>) {
        let Some(selected) = selected else { return };
        for p in &mut self.programs[start..] {
            if p.outcome == Outcome::Failed {
                p.outcome = Outcome::Fallback;
                p.selected = Some(selected.into());
            }
        }
        if requested != selected {
            self.record(
                requested,
                Phase::Selection,
                Outcome::Fallback,
                "documented program fallback",
            );
            self.programs.last_mut().unwrap().selected = Some(selected.into());
        }
    }

    pub(crate) fn preprocessing(&mut self, name: &str, stages: &[&crate::Preprocessed]) {
        for pp in stages {
            for d in &pp.diagnostics {
                self.record(name, Phase::Preprocessing, Outcome::Failed, &d.message);
                let p = self.programs.last_mut().unwrap();
                p.file = pp.files.get(d.origin.file as usize).cloned();
                p.line = Some(d.origin.line);
            }
        }
    }

    pub(crate) fn translation(&mut self, name: &str, errors: &[crate::Error]) {
        for e in errors {
            self.record(name, Phase::Translation, Outcome::Failed, &e.message);
            let p = self.programs.last_mut().unwrap();
            p.file = Some(e.file.clone());
            p.line = Some(e.line);
        }
    }
}
