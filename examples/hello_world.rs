use emergent::{BehaviorSpec, Engine, EngineConfig, EngineContext, EntityId, Game, Vec3};

#[derive(Default)]
struct HelloWorld {
    mover: Option<EntityId>,
}

impl Game for HelloWorld {
    fn start(&mut self, context: &mut EngineContext<'_>) {
        let scene = context.world.spawn("Hello scene");
        let mover = context
            .world
            .spawn_child(scene, "Hello mover")
            .expect("the scene accepts its child");
        context
            .world
            .add_behavior(
                mover,
                BehaviorSpec::linear_motion(Vec3::ZERO, Vec3::new(1.0, 0.0, 0.0))
                    .expect("finite motion"),
            )
            .expect("the entity accepts its motion behaviour");
        self.mover = Some(mover);
        println!("Hello, Emergent!");
    }

    fn fixed_update(&mut self, context: &mut EngineContext<'_>) {
        if context.tick >= 119 {
            context.request_exit();
        }
    }

    fn after_fixed_update(&mut self, context: &mut EngineContext<'_>) {
        if context.tick.is_multiple_of(30) {
            let position = context
                .world
                .global_transform(self.mover.expect("the game has started"))
                .expect("the mover remains alive")
                .translation();
            println!("tick {:>3}: x = {:.2}", context.tick, position.x);
        }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    Engine::new(EngineConfig::default())?.run(&mut HelloWorld::default())?;
    Ok(())
}
