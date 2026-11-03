use std::error::Error;
use std::fmt;
use std::fs;
use std::path::{Component, Path, PathBuf};

use serde::Deserialize;

pub const PROJECT_SCHEMA_VERSION: u32 = 1;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum TargetOs {
    Windows,
    Linux,
    Macos,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum BuildProfile {
    Development,
    Release,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ProjectOutput {
    pub id: String,
    pub os: TargetOs,
    pub architecture: String,
    pub rust_target: String,
    pub profile: BuildProfile,
    #[serde(default = "enabled_by_default")]
    pub enabled: bool,
}

fn enabled_by_default() -> bool {
    true
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ProjectManifest {
    pub schema_version: u32,
    pub id: String,
    pub name: String,
    pub entry_scene: PathBuf,
    pub generated_code: PathBuf,
    pub asset_roots: Vec<PathBuf>,
    pub outputs: Vec<ProjectOutput>,
    /// Plugins this project uses, each a folder holding a `plugin.toml`.
    #[serde(default)]
    pub plugins: Vec<ProjectPlugin>,
}

/// A `[[plugins]]` entry in `emergent.toml`.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ProjectPlugin {
    pub id: String,
    /// Project-relative plugin folder, conventionally `plugins/<id>`.
    pub path: PathBuf,
}

pub const PLUGIN_SCHEMA_VERSION: u32 = 1;

/// A plugin's `plugin.toml`: identity, version and what it contributes.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct PluginManifest {
    pub schema_version: u32,
    /// Durable identity and component-ID prefix, e.g. `acme.beans`.
    pub id: String,
    pub name: String,
    /// Semver `major.minor.patch`.
    pub version: String,
    #[serde(default)]
    pub description: String,
    /// Cargo package of the plugin's code; absent for an asset-only plugin.
    #[serde(rename = "crate")]
    pub crate_name: Option<String>,
    /// Asset folders relative to the plugin folder.
    #[serde(default)]
    pub asset_roots: Vec<PathBuf>,
    /// Project-relative plugin folder (from the project's `[[plugins]]` entry).
    #[serde(skip)]
    pub path: PathBuf,
}

#[derive(Clone, Debug)]
pub struct Project {
    root: PathBuf,
    manifest: ProjectManifest,
    plugins: Vec<PluginManifest>,
    /// The project's asset roots followed by each plugin's, project-relative.
    asset_roots: Vec<PathBuf>,
}

impl Project {
    pub fn load(root: impl AsRef<Path>) -> Result<Self, ProjectError> {
        let requested_root = root.as_ref();
        let root = requested_root
            .canonicalize()
            .map_err(|error| ProjectError::Io {
                path: requested_root.to_path_buf(),
                message: error.to_string(),
            })?;
        let manifest_path = root.join("emergent.toml");
        let source = fs::read_to_string(&manifest_path).map_err(|error| ProjectError::Io {
            path: manifest_path.clone(),
            message: error.to_string(),
        })?;
        let manifest =
            toml::from_str::<ProjectManifest>(&source).map_err(|error| ProjectError::Parse {
                path: manifest_path,
                message: error.to_string(),
            })?;
        validate_manifest(&root, &manifest)?;
        let plugins = load_plugins(&root, &manifest.plugins)?;
        let mut asset_roots = manifest.asset_roots.clone();
        for plugin in &plugins {
            asset_roots.extend(plugin.asset_roots.iter().map(|r| plugin.path.join(r)));
        }
        Ok(Self {
            root,
            manifest,
            plugins,
            asset_roots,
        })
    }

    /// Validated manifests of the plugins listed in `emergent.toml`, in listed order.
    pub fn plugins(&self) -> &[PluginManifest] {
        &self.plugins
    }

    /// Every folder assets may load from: the project's roots, then plugin roots.
    pub fn asset_roots(&self) -> &[PathBuf] {
        &self.asset_roots
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn manifest(&self) -> &ProjectManifest {
        &self.manifest
    }

    pub fn entry_scene_path(&self) -> PathBuf {
        self.root.join(&self.manifest.entry_scene)
    }

    pub fn enabled_outputs(&self) -> impl Iterator<Item = &ProjectOutput> {
        self.manifest.outputs.iter().filter(|output| output.enabled)
    }

    pub fn asset_path(&self, relative: impl AsRef<Path>) -> Result<PathBuf, ProjectError> {
        let relative = relative.as_ref();
        validate_relative_path("asset", relative)?;
        if !self
            .asset_roots
            .iter()
            .any(|root| relative.starts_with(root))
        {
            return Err(ProjectError::OutsideAssetRoots(relative.to_path_buf()));
        }
        let path = self.root.join(relative);
        canonical_path_within_root(&self.root, "asset", &path)?;
        Ok(path)
    }

    pub fn read_asset(&self, relative: impl AsRef<Path>) -> Result<Vec<u8>, ProjectError> {
        let path = self.asset_path(relative)?;
        fs::read(&path).map_err(|error| ProjectError::Io {
            path,
            message: error.to_string(),
        })
    }

    pub fn read_text_asset(&self, relative: impl AsRef<Path>) -> Result<String, ProjectError> {
        let path = self.asset_path(relative)?;
        fs::read_to_string(&path).map_err(|error| ProjectError::Io {
            path,
            message: error.to_string(),
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProjectError {
    Io { path: PathBuf, message: String },
    Parse { path: PathBuf, message: String },
    UnsupportedSchema { found: u32, supported: u32 },
    InvalidIdentifier { field: &'static str, value: String },
    InvalidPath { field: &'static str, path: PathBuf },
    MissingPath { field: &'static str, path: PathBuf },
    PathEscapesProject { field: &'static str, path: PathBuf },
    OutsideAssetRoots(PathBuf),
    DuplicateOutputId(String),
    Plugin { id: String, message: String },
    NoAssetRoots,
    NoEnabledOutputs,
}

impl fmt::Display for ProjectError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io { path, message } => write!(formatter, "{}: {message}", path.display()),
            Self::Parse { path, message } => {
                write!(formatter, "could not parse {}: {message}", path.display())
            }
            Self::UnsupportedSchema { found, supported } => write!(
                formatter,
                "project schema version {found} is unsupported; this engine supports {supported}"
            ),
            Self::InvalidIdentifier { field, value } => {
                write!(formatter, "{field} has invalid identifier '{value}'")
            }
            Self::InvalidPath { field, path } => write!(
                formatter,
                "{field} path '{}' must be a portable project-relative path",
                path.display()
            ),
            Self::MissingPath { field, path } => {
                write!(
                    formatter,
                    "{field} path '{}' does not exist",
                    path.display()
                )
            }
            Self::PathEscapesProject { field, path } => write!(
                formatter,
                "{field} path '{}' resolves outside the project root",
                path.display()
            ),
            Self::OutsideAssetRoots(path) => write!(
                formatter,
                "asset '{}' is not inside a declared asset root",
                path.display()
            ),
            Self::DuplicateOutputId(id) => write!(formatter, "duplicate output id '{id}'"),
            Self::Plugin { id, message } => write!(formatter, "plugin '{id}': {message}"),
            Self::NoAssetRoots => {
                formatter.write_str("project must declare at least one asset root")
            }
            Self::NoEnabledOutputs => {
                formatter.write_str("project must enable at least one build output")
            }
        }
    }
}

impl Error for ProjectError {}

fn validate_manifest(root: &Path, manifest: &ProjectManifest) -> Result<(), ProjectError> {
    if manifest.schema_version != PROJECT_SCHEMA_VERSION {
        return Err(ProjectError::UnsupportedSchema {
            found: manifest.schema_version,
            supported: PROJECT_SCHEMA_VERSION,
        });
    }
    validate_identifier("project id", &manifest.id)?;
    if manifest.name.trim().is_empty() {
        return Err(ProjectError::InvalidIdentifier {
            field: "project name",
            value: manifest.name.clone(),
        });
    }
    if manifest.asset_roots.is_empty() {
        return Err(ProjectError::NoAssetRoots);
    }

    validate_declared_path(root, "generated code", &manifest.generated_code, true)?;
    for asset_root in &manifest.asset_roots {
        validate_declared_path(root, "asset root", asset_root, true)?;
    }
    validate_declared_path(root, "entry scene", &manifest.entry_scene, false)?;
    if !manifest
        .asset_roots
        .iter()
        .any(|asset_root| manifest.entry_scene.starts_with(asset_root))
    {
        return Err(ProjectError::OutsideAssetRoots(
            manifest.entry_scene.clone(),
        ));
    }

    let mut output_ids = std::collections::HashSet::with_capacity(manifest.outputs.len());
    let mut enabled = 0;
    for output in &manifest.outputs {
        validate_identifier("output id", &output.id)?;
        if !output_ids.insert(output.id.as_str()) {
            return Err(ProjectError::DuplicateOutputId(output.id.clone()));
        }
        validate_identifier("output architecture", &output.architecture)?;
        validate_identifier("output Rust target", &output.rust_target)?;
        enabled += usize::from(output.enabled);
    }
    if enabled == 0 {
        return Err(ProjectError::NoEnabledOutputs);
    }
    Ok(())
}

fn load_plugins(
    root: &Path,
    entries: &[ProjectPlugin],
) -> Result<Vec<PluginManifest>, ProjectError> {
    let mut plugins: Vec<PluginManifest> = Vec::with_capacity(entries.len());
    for entry in entries {
        let fail = |message: String| ProjectError::Plugin {
            id: entry.id.clone(),
            message,
        };
        if !crate::plugin::valid_id(&entry.id) {
            return Err(fail(
                "id must look like publisher.name (lowercase, not emergent or project)".into(),
            ));
        }
        if plugins.iter().any(|plugin| plugin.id == entry.id) {
            return Err(fail("is listed twice".into()));
        }
        validate_declared_path(root, "plugin folder", &entry.path, true)?;
        let manifest_path = root.join(&entry.path).join("plugin.toml");
        let source = fs::read_to_string(&manifest_path).map_err(|error| ProjectError::Io {
            path: manifest_path.clone(),
            message: error.to_string(),
        })?;
        let mut plugin =
            toml::from_str::<PluginManifest>(&source).map_err(|error| ProjectError::Parse {
                path: manifest_path,
                message: error.to_string(),
            })?;
        if plugin.schema_version != PLUGIN_SCHEMA_VERSION {
            return Err(fail(format!(
                "plugin.toml schema version {} is unsupported; this engine supports {PLUGIN_SCHEMA_VERSION}",
                plugin.schema_version
            )));
        }
        if plugin.id != entry.id {
            return Err(fail(format!("plugin.toml declares id '{}'", plugin.id)));
        }
        if plugin.name.trim().is_empty() {
            return Err(fail("plugin.toml needs a name".into()));
        }
        let semver = plugin.version.split('.').collect::<Vec<_>>();
        if semver.len() != 3
            || !semver
                .iter()
                .all(|part| !part.is_empty() && part.bytes().all(|b| b.is_ascii_digit()))
        {
            return Err(fail(format!(
                "version '{}' must be major.minor.patch",
                plugin.version
            )));
        }
        if plugin.crate_name.as_deref().is_some_and(|name| {
            name.is_empty()
                || !name.bytes().all(|b| {
                    b.is_ascii_lowercase() || b.is_ascii_digit() || matches!(b, b'-' | b'_')
                })
        }) {
            return Err(fail("crate must be a lowercase Cargo package name".into()));
        }
        for asset_root in &plugin.asset_roots {
            validate_declared_path(
                root,
                "plugin asset root",
                &entry.path.join(asset_root),
                true,
            )?;
        }
        plugin.path = entry.path.clone();
        plugins.push(plugin);
    }
    Ok(plugins)
}

fn validate_identifier(field: &'static str, value: &str) -> Result<(), ProjectError> {
    if value.is_empty()
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
    {
        return Err(ProjectError::InvalidIdentifier {
            field,
            value: value.to_owned(),
        });
    }
    Ok(())
}

fn validate_relative_path(field: &'static str, path: &Path) -> Result<(), ProjectError> {
    let text = path.to_string_lossy();
    let valid = !text.is_empty()
        && !text.contains(['\\', ':'])
        && path
            .components()
            .all(|component| matches!(component, Component::Normal(_)));
    if valid {
        Ok(())
    } else {
        Err(ProjectError::InvalidPath {
            field,
            path: path.to_path_buf(),
        })
    }
}

fn validate_declared_path(
    root: &Path,
    field: &'static str,
    relative: &Path,
    require_directory: bool,
) -> Result<(), ProjectError> {
    validate_relative_path(field, relative)?;
    let path = root.join(relative);
    let resolved = canonical_path_within_root(root, field, &path)?;
    if require_directory && !resolved.is_dir() {
        return Err(ProjectError::MissingPath { field, path });
    }
    if !require_directory && !resolved.is_file() {
        return Err(ProjectError::MissingPath { field, path });
    }
    Ok(())
}

fn canonical_path_within_root(
    root: &Path,
    field: &'static str,
    path: &Path,
) -> Result<PathBuf, ProjectError> {
    let resolved = path.canonicalize().map_err(|_| ProjectError::MissingPath {
        field,
        path: path.to_path_buf(),
    })?;
    if !resolved.starts_with(root) {
        return Err(ProjectError::PathEscapesProject {
            field,
            path: path.to_path_buf(),
        });
    }
    Ok(resolved)
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicU64, Ordering};

    use super::*;

    static NEXT_FIXTURE: AtomicU64 = AtomicU64::new(1);

    struct Fixture(PathBuf);

    impl Fixture {
        fn new(manifest: &str) -> Self {
            let root = std::env::temp_dir().join(format!(
                "emergent-project-test-{}-{}",
                std::process::id(),
                NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir_all(root.join("src/generated")).unwrap();
            fs::create_dir_all(root.join("assets/scenes")).unwrap();
            fs::create_dir_all(root.join("assets/data")).unwrap();
            fs::write(
                root.join("assets/scenes/main.scene.toml"),
                "schema_version = 1",
            )
            .unwrap();
            fs::write(root.join("assets/data/message.txt"), "hello project").unwrap();
            fs::write(root.join("emergent.toml"), manifest).unwrap();
            Self(root)
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn manifest(extra: &str) -> String {
        format!(
            r#"schema_version = 1
id = "test.game"
name = "Test Game"
entry_scene = "assets/scenes/main.scene.toml"
generated_code = "src/generated"
asset_roots = ["assets"]

[[outputs]]
id = "host-dev"
os = "macos"
architecture = "aarch64"
rust_target = "aarch64-apple-darwin"
profile = "development"
{extra}
"#
        )
    }

    #[test]
    fn loads_valid_project_and_reads_declared_asset() {
        let fixture = Fixture::new(&manifest(""));
        let project = Project::load(&fixture.0).unwrap();
        assert_eq!(project.manifest().id, "test.game");
        assert_eq!(project.enabled_outputs().count(), 1);
        assert_eq!(
            project.read_text_asset("assets/data/message.txt").unwrap(),
            "hello project"
        );
    }

    #[test]
    fn rejects_unsupported_schema_and_unsafe_paths() {
        let fixture =
            Fixture::new(&manifest("").replace("schema_version = 1", "schema_version = 2"));
        assert_eq!(
            Project::load(&fixture.0).unwrap_err(),
            ProjectError::UnsupportedSchema {
                found: 2,
                supported: 1,
            }
        );

        let fixture = Fixture::new(&manifest("").replace("src/generated", "../generated"));
        assert!(matches!(
            Project::load(&fixture.0),
            Err(ProjectError::InvalidPath {
                field: "generated code",
                ..
            })
        ));
    }

    #[test]
    fn rejects_missing_paths_duplicate_outputs_and_no_enabled_output() {
        let fixture = Fixture::new(&manifest("").replace(
            "assets/scenes/main.scene.toml",
            "assets/scenes/missing.toml",
        ));
        assert!(matches!(
            Project::load(&fixture.0),
            Err(ProjectError::MissingPath {
                field: "entry scene",
                ..
            })
        ));

        let duplicate = format!(
            "{}\n[[outputs]]\nid = \"host-dev\"\nos = \"linux\"\narchitecture = \"x86_64\"\nrust_target = \"x86_64-unknown-linux-gnu\"\nprofile = \"release\"\n",
            manifest("")
        );
        let fixture = Fixture::new(&duplicate);
        assert_eq!(
            Project::load(&fixture.0).unwrap_err(),
            ProjectError::DuplicateOutputId("host-dev".into())
        );

        let fixture = Fixture::new(&manifest("enabled = false"));
        assert_eq!(
            Project::load(&fixture.0).unwrap_err(),
            ProjectError::NoEnabledOutputs
        );
    }

    #[test]
    fn refuses_assets_outside_declared_roots() {
        let fixture = Fixture::new(&manifest(""));
        fs::write(fixture.0.join("outside.txt"), "not an asset").unwrap();
        let project = Project::load(&fixture.0).unwrap();
        assert_eq!(
            project.asset_path("outside.txt").unwrap_err(),
            ProjectError::OutsideAssetRoots(PathBuf::from("outside.txt"))
        );
    }
}
