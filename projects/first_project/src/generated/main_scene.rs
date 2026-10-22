//! Generated from the project's initial scene design.
//!
//! Authored scene loading is not implemented yet, so this module is the
//! executable source of truth for runtime assembly in the first project slice.

use emergent::{behavior_ports, BehaviorSpec, EntityId, Vec3, World, WorldError};

#[derive(Clone, Copy, Debug)]
pub struct MainScene {
    pub leader: EntityId,
    pub tail: EntityId,
}

pub fn build_main_scene(world: &mut World, leader_speed: f32) -> Result<MainScene, WorldError> {
    let scene = world.spawn("First Project Scene");
    let leader = world.spawn_child(scene, "Generated leader")?;
    let follower = world.spawn_child(scene, "Generated follower")?;
    let tail = world.spawn_child(scene, "Generated tail")?;

    let leader_motion = world
        .add_behavior(
            leader,
            BehaviorSpec::linear_motion(Vec3::ZERO, Vec3::new(leader_speed, 0.0, 0.0))
                .expect("project data is finite"),
        )
        .expect("new leader accepts its behaviour");
    let follower_motion = world
        .add_behavior(
            follower,
            BehaviorSpec::follow_position(Vec3::new(-2.0, 0.0, 0.0), 2.0)
                .expect("generated follower speed is valid"),
        )
        .expect("new follower accepts its behaviour");
    let tail_motion = world
        .add_behavior(
            tail,
            BehaviorSpec::follow_position(Vec3::new(-4.0, 0.0, 0.0), 2.0)
                .expect("generated follower speed is valid"),
        )
        .expect("new tail accepts its behaviour");

    world
        .connect_behaviors(
            leader_motion,
            behavior_ports::POSITION,
            follower_motion,
            behavior_ports::TARGET,
        )
        .expect("generated position link is type-correct");
    world
        .connect_behaviors(
            follower_motion,
            behavior_ports::POSITION,
            tail_motion,
            behavior_ports::TARGET,
        )
        .expect("generated position link is type-correct");

    Ok(MainScene { leader, tail })
}
