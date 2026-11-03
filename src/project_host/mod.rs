//! The game-project host: the command-line contract the IDE and tools drive
//! (`--inspect-scene`, `--validate-scenes`, `--export-components`, previews, brain and locomotion tools)
//! and the native window. A game implements [`ProjectGame`] and calls [`main`]; new
//! IDE-facing features belong here so every project gets them without code changes.
use std::error::Error;
use std::io::Read;

use crate::component::ComponentRegistry;
use crate::plugin::{Plugin, PluginRuntimes};
use crate::{Engine, EngineConfig, EngineContext, Game, LoadedScene, Project, SceneDefinition};

mod audio_tools;
mod brain_tools;
mod inspection;
mod inspection_tools;
mod locomotion_tools;
mod map_tools;
mod scene_tools;
mod visual;

/// A game the host can load into an engine for play, inspection and previews.
pub trait ProjectGame: Game + Sized {
    /// Plugins compiled into this game. The host registers their component types,
    /// attaches their runtime after `load` and advances it after each `fixed_update`.
    fn plugins() -> &'static [&'static dyn Plugin] {
        &[]
    }
    /// Component types for loading and validation: engine built-ins plus plugins.
    /// To add `project.*` types, start from `plugin::registry(Self::plugins())`.
    fn components() -> ComponentRegistry {
        crate::plugin::registry(Self::plugins())
            .expect("project_host::main validated the plugins before loading")
    }
    /// Loads `scene` (a project-relative path) and wires the game's behaviour onto it.
    fn load(engine: &mut Engine, project: &Project, scene: &str) -> Result<Self, Box<dyn Error>>;
    /// The scene this game loaded; the host renders and inspects its objects.
    fn loaded(&self) -> &LoadedScene;
}

/// A game plus the runtime of its plugins; every host mode runs games through this.
pub(crate) struct Plugged<G> {
    inner: G,
    plugins: PluginRuntimes,
}

impl<G: Game> Game for Plugged<G> {
    fn start(&mut self, context: &mut EngineContext<'_>) {
        self.inner.start(context);
    }
    fn fixed_update(&mut self, context: &mut EngineContext<'_>) {
        self.inner.fixed_update(context);
        if let Err(error) = self.plugins.fixed_update(context) {
            // shortcut: Game has no error channel, so the process still exits 0; add one when hosts report game failures.
            eprintln!("Stopping: {error}");
            context.request_exit();
        }
    }
    fn after_fixed_update(&mut self, context: &mut EngineContext<'_>) {
        self.inner.after_fixed_update(context);
    }
    fn stop(&mut self, context: &mut EngineContext<'_>) {
        self.inner.stop(context);
    }
}

impl<G: ProjectGame> ProjectGame for Plugged<G> {
    fn plugins() -> &'static [&'static dyn Plugin] {
        G::plugins()
    }
    fn components() -> ComponentRegistry {
        G::components()
    }
    fn load(engine: &mut Engine, project: &Project, scene: &str) -> Result<Self, Box<dyn Error>> {
        let inner = G::load(engine, project, scene)?;
        let plugins = PluginRuntimes::attach(G::plugins(), inner.loaded(), engine.world_mut())?;
        Ok(Self { inner, plugins })
    }
    fn loaded(&self) -> &LoadedScene {
        self.inner.loaded()
    }
}

/// Stops a game after a fixed number of ticks, for smoke runs.
pub(crate) struct TickLimit<G> {
    pub(crate) inner: G,
    limit: Option<u64>,
}

impl<G> TickLimit<G> {
    pub(crate) fn new(inner: G, limit: Option<u64>) -> Self {
        Self { inner, limit }
    }
}

impl<G: Game> Game for TickLimit<G> {
    fn start(&mut self, context: &mut EngineContext<'_>) {
        self.inner.start(context);
    }
    fn fixed_update(&mut self, context: &mut EngineContext<'_>) {
        self.inner.fixed_update(context);
        if self
            .limit
            .is_some_and(|limit| context.tick.saturating_add(1) >= limit)
        {
            context.request_exit();
        }
    }
    fn after_fixed_update(&mut self, context: &mut EngineContext<'_>) {
        self.inner.after_fixed_update(context);
    }
    fn stop(&mut self, context: &mut EngineContext<'_>) {
        self.inner.stop(context);
    }
}

const USAGE: &str = "usage: [--smoke | --headless | --run-scene <scene> [--smoke] | --preview-scene <scene> --preview-output <png> | --inspect-scene <scene> | --validate-scene <scene> [--candidate <file>] | --validate-scenes <scene>... | --inspect-audio <path> [--candidate <file>] | --export-components | --scene-transforms <scene> | --map-layout <scene> | --convert-global <scene> <id> <file> [--position-only] | --export-brain-nodes | --validate-brain ... | --simulate-brain ... | --export-ui-widgets | --validate-ui <path> [--candidate <file>] | --export-locomotion | --validate-locomotion ... | --simulate-locomotion ...]";

/// Runs the project named by `manifest_dir` (pass `env!("CARGO_MANIFEST_DIR")`) using the
/// process arguments. With no arguments it opens the entry scene in a window.
pub fn main<G: ProjectGame>(manifest_dir: &str) -> Result<(), Box<dyn Error>> {
    let project = Project::load(manifest_dir)?;
    // Report bad plugins as an error here so `components()` can rely on them later.
    crate::plugin::registry(G::plugins())?;
    check_plugins(&project, G::plugins())?;
    dispatch::<Plugged<G>>(project)
}

/// The compiled plugins must be exactly the `emergent.toml` plugins that have code.
fn check_plugins(project: &Project, compiled: &[&dyn Plugin]) -> Result<(), String> {
    for listed in project.plugins() {
        if listed.crate_name.is_some() && !compiled.iter().any(|p| p.id() == listed.id) {
            return Err(format!(
                "plugin '{}' is listed in emergent.toml but not compiled into this game: \
                 regenerate src/generated/plugins.rs, return it from ProjectGame::plugins() and rebuild",
                listed.id
            ));
        }
    }
    for plugin in compiled {
        let id = plugin.id();
        match project.plugins().iter().find(|listed| listed.id == id) {
            None => {
                return Err(format!(
                    "plugin '{id}' is compiled in but not listed under [[plugins]] in emergent.toml"
                ))
            }
            Some(listed) if listed.crate_name.is_none() => {
                return Err(format!(
                    "plugin '{id}' is compiled in but its plugin.toml declares no crate"
                ))
            }
            Some(_) => {}
        }
    }
    Ok(())
}

fn dispatch<G: ProjectGame>(project: Project) -> Result<(), Box<dyn Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let registry = G::components;
    let entry = || -> Result<String, Box<dyn Error>> {
        Ok(project
            .manifest()
            .entry_scene
            .to_str()
            .ok_or("non-UTF8 scene path")?
            .to_owned())
    };
    match args
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>()
        .as_slice()
    {
        ["--export-brain-nodes"] => {
            print!("{}", crate::brain::export_node_catalogue_with(&registry())?);
            Ok(())
        }
        ["--validate-brain", rest @ ..] => {
            brain_tools::validate(&project, &registry(), &args[args.len() - rest.len()..])
        }
        ["--simulate-brain", rest @ ..] => {
            brain_tools::preview(&project, &registry(), &args[args.len() - rest.len()..])
        }
        ["--export-ui-widgets"] => {
            print!("{}", crate::ui::export_widget_catalogue()?);
            Ok(())
        }
        ["--validate-ui", path] => {
            let ui = crate::ui::UiDefinition::load(&project, path)?;
            println!("UI '{}' validated: {} nodes", ui.name, ui.nodes.len());
            Ok(())
        }
        ["--validate-ui", path, "--candidate", file] => {
            let mut source = String::new();
            std::fs::File::open(file)?
                .take(256 * 1024 + 1)
                .read_to_string(&mut source)?;
            let ui = crate::ui::UiDefinition::from_source(path, &source)?;
            println!("UI '{}' validated: {} nodes", ui.name, ui.nodes.len());
            Ok(())
        }
        ["--export-locomotion"] => {
            print!(
                "{}",
                crate::locomotion::export_type_catalogue_with(&registry())?
            );
            Ok(())
        }
        ["--validate-locomotion", rest @ ..] => {
            locomotion_tools::validate(&project, &registry(), &args[args.len() - rest.len()..])
        }
        ["--simulate-locomotion", rest @ ..] => {
            locomotion_tools::preview(&project, &registry(), &args[args.len() - rest.len()..])
        }
        ["--inspect-audio", rest @ ..] => {
            audio_tools::inspect(&project, &args[args.len() - rest.len()..])
        }
        ["--export-components"] => {
            print!("{}", registry().export_toml()?);
            Ok(())
        }
        ["--scene-transforms", scene] => scene_tools::report(&project, &registry(), scene),
        ["--map-layout", scene] => map_tools::layout(&project, &registry(), scene),
        ["--convert-global", scene, id, file] => {
            scene_tools::convert(&project, &registry(), scene, id, file, false)
        }
        ["--convert-global", scene, id, file, "--position-only"] => {
            scene_tools::convert(&project, &registry(), scene, id, file, true)
        }
        ["--validate-scenes", scenes @ ..] if !scenes.is_empty() && scenes.len() <= 128 => {
            for scene in scenes {
                if scene.ends_with(".prefab.toml") {
                    crate::PrefabDefinition::load_with(&project, scene, &registry())?;
                } else {
                    SceneDefinition::load_with(&project, scene, &registry())?;
                }
            }
            println!("Scene candidates validated");
            Ok(())
        }
        ["--inspect-scene", scene] => inspection::run::<G>(&project, scene),
        ["--validate-scene", scene] => {
            if scene.ends_with(".prefab.toml") {
                crate::PrefabDefinition::load_with(&project, scene, &registry())?;
            } else {
                SceneDefinition::load_with(&project, scene, &registry())?;
            }
            println!("Scene candidate validated without construction or simulation");
            Ok(())
        }
        ["--validate-scene", scene, "--candidate", file] => {
            let mut source = String::new();
            std::fs::File::open(file)?
                .take(crate::SCENE_MAX_BYTES + 1)
                .read_to_string(&mut source)?;
            if scene.ends_with(".prefab.toml") {
                crate::PrefabDefinition::from_source_with(&project, scene, &source, &registry())?;
            } else {
                SceneDefinition::from_source_with(&project, scene, &source, &registry())?;
            }
            println!("Scene candidate validated without construction or simulation");
            Ok(())
        }
        ["--run-scene", scene] => visual::run_scene::<G>(&project, None, scene),
        ["--run-scene", scene, "--smoke"] => visual::run_scene::<G>(&project, Some(120), scene),
        ["--preview-scene", scene, "--preview-output", output] => {
            visual::preview::<G>(&project, scene, std::path::Path::new(output))
        }
        [] => visual::run::<G>(&project, None),
        ["--smoke"] => visual::run::<G>(&project, Some(120)),
        ["--headless"] => {
            let mut engine = Engine::new(EngineConfig::default())?;
            let game = G::load(&mut engine, &project, &entry()?)?;
            engine.run(&mut TickLimit::new(game, Some(120)))?;
            Ok(())
        }
        _ => Err(USAGE.into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Named(&'static str);
    impl Plugin for Named {
        fn id(&self) -> &'static str {
            self.0
        }
    }

    #[test]
    fn compiled_plugins_must_match_the_project_list() {
        let root =
            std::env::temp_dir().join(format!("emergent-host-plugins-{}", std::process::id()));
        for dir in [
            "assets",
            "src/generated",
            "plugins/acme.code",
            "plugins/acme.art",
        ] {
            std::fs::create_dir_all(root.join(dir)).unwrap();
        }
        std::fs::write(root.join("emergent.toml"), "schema_version = 1\nid = 'fixture'\nname = 'Fixture'\nentry_scene = 'assets/main.scene.toml'\ngenerated_code = 'src/generated'\nasset_roots = ['assets']\n[[outputs]]\nid = 'host'\nos = 'macos'\narchitecture = 'aarch64'\nrust_target = 'aarch64-apple-darwin'\nprofile = 'development'\n[[plugins]]\nid = 'acme.code'\npath = 'plugins/acme.code'\n[[plugins]]\nid = 'acme.art'\npath = 'plugins/acme.art'\n").unwrap();
        std::fs::write(
            root.join("assets/main.scene.toml"),
            "schema_version=1\nid='main'\n",
        )
        .unwrap();
        std::fs::write(root.join("plugins/acme.code/plugin.toml"), "schema_version = 1\nid = 'acme.code'\nname = 'Code'\nversion = '1.0.0'\ncrate = 'acme-code'\n").unwrap();
        std::fs::write(
            root.join("plugins/acme.art/plugin.toml"),
            "schema_version = 1\nid = 'acme.art'\nname = 'Art'\nversion = '1.0.0'\n",
        )
        .unwrap();
        let project = Project::load(&root).unwrap();
        std::fs::remove_dir_all(&root).unwrap();

        // Asset-only plugins need no code; code plugins must be compiled in.
        assert!(check_plugins(&project, &[&Named("acme.code")]).is_ok());
        let missing = check_plugins(&project, &[]).unwrap_err();
        assert!(
            missing.contains("'acme.code' is listed") && missing.contains("rebuild"),
            "{missing}"
        );
        let unlisted =
            check_plugins(&project, &[&Named("acme.code"), &Named("acme.extra")]).unwrap_err();
        assert!(unlisted.contains("not listed"), "{unlisted}");
        let codeless =
            check_plugins(&project, &[&Named("acme.code"), &Named("acme.art")]).unwrap_err();
        assert!(codeless.contains("declares no crate"), "{codeless}");
    }
}
