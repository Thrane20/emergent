//! Generated from the project's initial scene design.
//!
//! Authored scene loading is not implemented yet, so this module is the
//! executable source of truth for runtime assembly in the first project slice.

use emergent::{
    behavior_ports, BehaviorSpec, Color, EntityId, LightSource, ShadowCasting, Transform, Vec3,
    World, WorldError,
};

#[derive(Clone, Copy, Debug)]
pub struct MainScene {
    pub leader: EntityId,
    pub follower: EntityId,
    pub tail: EntityId,
    pub key_light: EntityId,
    pub ground: EntityId,
    pub pyramid: EntityId,
    pub cube: EntityId,
}

pub fn build_main_scene(world: &mut World, leader_speed: f32) -> Result<MainScene, WorldError> {
    let scene = world.spawn("First Project Scene");
    let leader = world.spawn_child(scene, "Generated leader")?;
    let follower = world.spawn_child(scene, "Generated follower")?;
    let tail = world.spawn_child(scene, "Generated tail")?;
    let key_light = world.spawn_child(scene, "Generated key light")?;
    let ground = world.spawn_child(scene, "Generated ground")?;
    let pyramid = world.spawn_child(scene, "Generated pyramid preview")?;
    // Generated from assets/entities/cube.entity.toml; retain only its runtime handle.
    let cube = world.spawn_child(scene, "Cube")?;

    world.set_local_transform(
        ground,
        Transform::from_translation(Vec3::new(0.0, -1.0, 0.0)),
    )?;
    world.set_local_transform(
        pyramid,
        Transform::from_translation(Vec3::new(0.0, -1.0, -2.5)),
    )?;

    world.set_local_transform(
        key_light,
        Transform::from_translation(Vec3::new(0.0, 6.0, 4.0)),
    )?;
    let light = LightSource::point(Color::WHITE, 8.0, 24.0)
        .expect("generated light parameters are valid")
        .with_shadow_casting(ShadowCasting::Enabled)
        .expect("point lights may request shadows");
    world.insert_component(key_light, light)?;

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

    Ok(MainScene {
        leader,
        follower,
        tail,
        key_light,
        ground,
        pyramid,
        cube,
    })
}

#[cfg(test)]
mod tests {
    use emergent::{LightKind, LightSource, ShadowCasting, Vec3, World};

    use super::build_main_scene;

    #[test]
    fn generated_scene_contains_the_configured_key_light_and_authored_cube() {
        let mut world = World::new();
        let scene = build_main_scene(&mut world, 1.0).unwrap();
        let light = world
            .component::<LightSource>(scene.key_light)
            .unwrap()
            .copied()
            .expect("key-light entity carries its component");

        assert_eq!(light.kind(), LightKind::Point);
        assert_eq!(light.intensity(), 8.0);
        assert_eq!(light.range(), Some(24.0));
        assert_eq!(light.shadow_casting(), ShadowCasting::Enabled);
        assert_eq!(
            world.local_transform(scene.ground).unwrap().translation,
            Vec3::new(0.0, -1.0, 0.0)
        );
        assert_eq!(
            world.local_transform(scene.pyramid).unwrap().translation,
            Vec3::new(0.0, -1.0, -2.5)
        );
        assert_eq!(world.get(scene.cube).unwrap().name(), "Cube");
        assert_eq!(
            world.local_transform(scene.cube).unwrap().translation,
            Vec3::ZERO
        );
        assert_eq!(world.local_transform(scene.cube).unwrap().scale, Vec3::ONE);
    }
}
