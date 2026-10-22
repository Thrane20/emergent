use emergent::{
    behavior_ports, BehaviorSpec, Engine, EngineConfig, EngineContext, EntityId, Game, Vec3,
};

#[derive(Default)]
struct FirstGame {
    leader: Option<EntityId>,
    first_follower: Option<EntityId>,
    second_follower: Option<EntityId>,
}

impl Game for FirstGame {
    fn start(&mut self, context: &mut EngineContext<'_>) {
        let scene = context.world.spawn("First Scene");
        let leader = context
            .world
            .spawn_child(scene, "Leader")
            .expect("the new scene must accept a leader");
        let first_follower = context
            .world
            .spawn_child(scene, "First follower")
            .expect("the new scene must accept a follower");
        let second_follower = context
            .world
            .spawn_child(scene, "Second follower")
            .expect("the new scene must accept a follower");

        let leader_motion = context
            .world
            .add_behavior(
                leader,
                BehaviorSpec::linear_motion(Vec3::ZERO, Vec3::new(0.5, 0.0, 0.0))
                    .expect("the leader motion must be valid"),
            )
            .expect("the leader must accept a behaviour");
        let first_motion = context
            .world
            .add_behavior(
                first_follower,
                BehaviorSpec::follow_position(Vec3::new(-2.0, 0.0, 0.0), 1.5)
                    .expect("the follower speed must be valid"),
            )
            .expect("the first follower must accept a behaviour");
        let second_motion = context
            .world
            .add_behavior(
                second_follower,
                BehaviorSpec::follow_position(Vec3::new(-4.0, 0.0, 0.0), 1.5)
                    .expect("the follower speed must be valid"),
            )
            .expect("the second follower must accept a behaviour");

        context
            .world
            .connect_behaviors(
                leader_motion,
                behavior_ports::POSITION,
                first_motion,
                behavior_ports::TARGET,
            )
            .expect("the leader position must connect to the first follower");
        context
            .world
            .connect_behaviors(
                first_motion,
                behavior_ports::POSITION,
                second_motion,
                behavior_ports::TARGET,
            )
            .expect("the first follower must connect to the second follower");

        self.leader = Some(leader);
        self.first_follower = Some(first_follower);
        self.second_follower = Some(second_follower);

        println!(
            "Emergent launched with {} entities and {} linked behaviours",
            context.world.len(),
            context.world.behavior_count()
        );
    }

    fn fixed_update(&mut self, context: &mut EngineContext<'_>) {
        if context.tick >= 119 {
            context.request_exit();
        }
    }

    fn after_fixed_update(&mut self, context: &mut EngineContext<'_>) {
        if context.tick.is_multiple_of(60) {
            let position = |entity| {
                context
                    .world
                    .global_transform(entity)
                    .expect("the behaviour-driven entity must remain alive")
                    .translation()
                    .x
            };
            println!(
                "tick {:>3}: leader={:.2}, first={:.2}, second={:.2}",
                context.tick,
                position(self.leader.expect("the leader must exist")),
                position(self.first_follower.expect("the first follower must exist")),
                position(
                    self.second_follower
                        .expect("the second follower must exist")
                ),
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
    engine.run(&mut game)?;
    Ok(())
}
