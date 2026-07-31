use std::collections::BTreeSet;
use std::fmt;

const SELECTED_MANIFEST: &str = include_str!("../../generated/hare/effect-3.22.0.manifest.json");

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SelectedEffectInventory {
    pub package: &'static str,
    pub version: &'static str,
    pub source_tree_sha256: &'static str,
    pub artifact_tree_sha256: &'static str,
    pub exports: Vec<&'static str>,
    pub artifact_files: Vec<&'static str>,
    pub source_files: Vec<&'static str>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ManifestError {
    MissingScalar(&'static str),
    MissingSection(&'static str),
    MalformedSection(&'static str),
    Count {
        section: &'static str,
        expected: usize,
        actual: usize,
    },
    DuplicatePath(&'static str),
    SourceMissingFromArtifact(&'static str),
    WrongPin,
}

impl fmt::Display for ManifestError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for ManifestError {}

impl SelectedEffectInventory {
    pub fn load() -> Result<Self, ManifestError> {
        let inventory = Self {
            package: scalar("package")?,
            version: scalar("version")?,
            source_tree_sha256: scalar("sourceTreeSha256")?,
            artifact_tree_sha256: scalar("artifactTreeSha256")?,
            exports: object_keys("exports")?,
            artifact_files: array_paths("artifactFiles")?,
            source_files: array_paths("sourceFiles")?,
        };
        inventory.validate()?;
        Ok(inventory)
    }

    pub fn validate(&self) -> Result<(), ManifestError> {
        if self.package != "effect"
            || self.version != "3.22.0"
            || self.source_tree_sha256
                != "6c14f8bf293b7f0e57f1e3785171385c2a6d2780944e602c17207bf22f67e48b"
            || self.artifact_tree_sha256
                != "1d7dda8385f655400f0c31776d42270f935ac86db1a010c52e8445086fc407eb"
        {
            return Err(ManifestError::WrongPin);
        }
        for (section, expected, values) in [
            ("exports", 179, &self.exports),
            ("artifactFiles", 2_715, &self.artifact_files),
            ("sourceFiles", 362, &self.source_files),
        ] {
            if values.len() != expected {
                return Err(ManifestError::Count {
                    section,
                    expected,
                    actual: values.len(),
                });
            }
            let mut unique = BTreeSet::new();
            for value in values {
                if !unique.insert(*value) {
                    return Err(ManifestError::DuplicatePath(value));
                }
            }
        }
        let artifacts = self.artifact_files.iter().copied().collect::<BTreeSet<_>>();
        for source in &self.source_files {
            if !artifacts.contains(source) {
                return Err(ManifestError::SourceMissingFromArtifact(source));
            }
        }
        Ok(())
    }

    pub fn contains_artifact(&self, path: &str) -> bool {
        self.artifact_files
            .binary_search_by(|candidate| candidate.cmp(&path))
            .is_ok()
    }
}

fn scalar(name: &'static str) -> Result<&'static str, ManifestError> {
    let prefix = format!("  \"{name}\": \"");
    SELECTED_MANIFEST
        .lines()
        .find_map(|line| {
            line.strip_prefix(&prefix)
                .and_then(|value| value.strip_suffix(',').unwrap_or(value).strip_suffix('"'))
        })
        .ok_or(ManifestError::MissingScalar(name))
}

fn array_paths(name: &'static str) -> Result<Vec<&'static str>, ManifestError> {
    let start = format!("  \"{name}\": [");
    let mut lines = SELECTED_MANIFEST.lines().skip_while(|line| *line != start);
    if lines.next().is_none() {
        return Err(ManifestError::MissingSection(name));
    }
    let mut paths = Vec::new();
    for line in lines {
        if line == "  ]," || line == "  ]" {
            paths.sort_unstable();
            return Ok(paths);
        }
        if let Some(path) = line
            .strip_prefix("      \"path\": \"")
            .and_then(|path| path.strip_suffix("\","))
        {
            paths.push(path);
        }
    }
    Err(ManifestError::MalformedSection(name))
}

fn object_keys(name: &'static str) -> Result<Vec<&'static str>, ManifestError> {
    let start = format!("  \"{name}\": {{");
    let mut lines = SELECTED_MANIFEST.lines().skip_while(|line| *line != start);
    if lines.next().is_none() {
        return Err(ManifestError::MissingSection(name));
    }
    let mut keys = Vec::new();
    for line in lines {
        if line == "  }," || line == "  }" {
            keys.sort_unstable();
            return Ok(keys);
        }
        if let Some(key) = line
            .strip_prefix("    \"")
            .and_then(|line| line.strip_suffix("\": {"))
        {
            keys.push(key);
        }
    }
    Err(ManifestError::MalformedSection(name))
}
