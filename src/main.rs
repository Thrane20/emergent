use emergent::{Engine, EngineConfig, EngineContext, EntityId, Game, Vec3};

#[derive(Default)]
struct FirstGame {
    scout: Option<EntityId>,
}

impl Game for FirstGame {
    fn start(&mut self, context: &mut EngineContext<'_>) {
        let scene = context.world.spawn("First Scene");
        let scout = context
            .world
            .spawn_child(scene, "Scout")
            .expect("the new scene must accept a child");
        self.scout = Some(scout);

        println!("Emergent launched with {} entities", context.world.len());
    }

    fn fixed_update(&mut self, context: &mut EngineContext<'_>) {
        let scout = self
            .scout
            .expect("the game must be started before updating");
        {
            let entity = context
                .world
                .get_mut(scout)
                .expect("the scout must remain alive during this example");
            entity.transform_mut().translation = Vec3::new(context.tick as f32 * 0.05, 0.0, 0.0);
        }

        if context.tick.is_multiple_of(60) {
            let entity = context
                .world
                .get(scout)
                .expect("the scout must remain alive during this example");
            println!(
                "tick {:>3}: {} is at x={:.2}",
                context.tick,
                entity.name(),
                entity.transform().translation.x
            );
        }

        if context.tick >= 119 {
            context.request_exit();
        }
    }

    fn stop(&mut self, context: &mut EngineContext<'_>) {
        println!(
            "Emergent stopped cleanly after {} ticks with {} entities",
            context.tick,
            context.world.len()
        );
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut engine = Engine::new(EngineConfig::default())?;
    let mut game = FirstGame::default();
    engine.run(&mut game);
    Ok(())
}
