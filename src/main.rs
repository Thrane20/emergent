use emergent::{Engine, EngineConfig, EngineContext, EntityId, Game, Quat, Transform, Vec3};

#[derive(Default)]
struct FirstGame {
    carrier: Option<EntityId>,
    scout: Option<EntityId>,
}

impl Game for FirstGame {
    fn start(&mut self, context: &mut EngineContext<'_>) {
        let scene = context.world.spawn("First Scene");
        let carrier = context
            .world
            .spawn_child(scene, "Carrier")
            .expect("the new scene must accept a carrier");
        let scout = context
            .world
            .spawn_child(carrier, "Scout")
            .expect("the carrier must accept a scout");
        context
            .world
            .set_local_transform(
                scout,
                Transform::from_translation(Vec3::new(0.0, 0.0, -2.0)),
            )
            .expect("the scout transform must be valid");
        self.carrier = Some(carrier);
        self.scout = Some(scout);

        println!("Emergent launched with {} entities", context.world.len());
    }

    fn fixed_update(&mut self, context: &mut EngineContext<'_>) {
        let carrier = self
            .carrier
            .expect("the game must be started before updating");
        let transform = Transform::new(
            Vec3::new(context.tick as f32 * 0.05, 0.0, 0.0),
            Quat::from_rotation_y(context.tick as f32 * 0.01),
            Vec3::ONE,
        );
        context
            .world
            .set_local_transform(carrier, transform)
            .expect("the generated carrier transform must be valid");

        if context.tick >= 119 {
            context.request_exit();
        }
    }

    fn after_fixed_update(&mut self, context: &mut EngineContext<'_>) {
        if context.tick.is_multiple_of(60) {
            let scout = self.scout.expect("the scout must exist while running");
            let position = context
                .world
                .global_transform(scout)
                .expect("the scout must remain alive")
                .translation();
            println!(
                "tick {:>3}: Scout world position = ({:.2}, {:.2}, {:.2})",
                context.tick, position.x, position.y, position.z
            );
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
