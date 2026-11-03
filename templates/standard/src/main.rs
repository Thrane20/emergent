//! A game project on Emergent. The engine's project host supplies the window, IDE
//! inspection, previews and tool commands; this file only describes the game.
use std::error::Error;

use emergent::project_host::ProjectGame;
use emergent::{Engine, EngineContext, Game, LoadedScene, Plugin, Project, SceneDefinition};

mod generated;

struct ProjectMain {
    loaded: LoadedScene,
}

impl Game for ProjectMain {
    /// Runs once per fixed tick. Built-in components (lights, character controller,
    /// spin, follow, brains, physics) already run inside the engine.
    fn fixed_update(&mut self, _context: &mut EngineContext<'_>) {}
}

impl ProjectGame for ProjectMain {
    /// Plugins under `plugins/`, listed in emergent.toml; the IDE regenerates the list.
    fn plugins() -> &'static [&'static dyn Plugin] {
        generated::plugins::PLUGINS
    }

    // To add project component types, override `components()`: start from
    // `emergent::plugin::registry(Self::plugins())` (`main` already validated it) and `register` each `project.<name>` type.

    fn load(engine: &mut Engine, project: &Project, scene: &str) -> Result<Self, Box<dyn Error>> {
        let loaded = SceneDefinition::load_with(project, scene, &Self::components())?
            .instantiate(engine.world_mut())?;
        Ok(Self { loaded })
    }

    fn loaded(&self) -> &LoadedScene {
        &self.loaded
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    emergent::project_host::main::<ProjectMain>(env!("CARGO_MANIFEST_DIR"))
}
