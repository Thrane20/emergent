use emergent::{EngineContext, Game};

use crate::avoidance::DistanceAvoidance;
use crate::bounce::{GroundBounce, PeriodicPop};
use crate::generated::main_scene::{build_main_scene, MainScene};
use crate::spin::{rotate_z, SLOW_SPIN_DEGREES_PER_SECOND};

const CENTER_CUBE_ID: &str = "f33d8ddb-5cd9-4512-a8fa-fa3081e1f99d";
const CENTER_CUBE_SPIN_DEGREES_PER_SECOND: f32 = 90.0;
const DEFENDER_ID: &str = "a5be312c-a7b9-4d9b-9eff-5be691a320b2";
const OFFENDER_BOX_ID: &str = "c1836e8d-78b5-4d90-9d59-f22cec9ac856";
const GROUND_ID: &str = "ground";
const FALLING_CUBE_ID: &str = "falling-cube";

pub struct FirstProjectGame {
    pub loaded: emergent::LoadedScene,
    scene: Option<MainScene>,
    avoidance: Option<DistanceAvoidance>,
    bounce: Option<GroundBounce>,
    jumping_bean: Option<PeriodicPop>,
    exit_after_ticks: Option<u64>,
}

impl FirstProjectGame {
    pub fn new(
        engine: &mut emergent::Engine,
        project: &emergent::Project,
        path: &str,
        leader_speed: f32,
        exit_after_ticks: Option<u64>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        if !leader_speed.is_finite() {
            return Err("leader speed must be finite".into());
        }
        let definition = emergent::SceneDefinition::load(project, path)?;
        let chain = match definition.builder() {
            Some("emergent.first-project.generated.main_scene") => true,
            None => false,
            Some(other) => {
                return Err(format!("unregistered project scene builder '{other}'").into())
            }
        };
        let mut loaded = definition.instantiate(engine.world_mut())?;
        let authored_pair = loaded
            .entities
            .get(DEFENDER_ID)
            .copied()
            .zip(loaded.entities.get(OFFENDER_BOX_ID).copied());
        let scene = if chain {
            let (defender, offender) = authored_pair.ok_or(
                "registered main scene requires authored Defender and OffenderBox instances",
            )?;
            let scene = build_main_scene(
                engine.world_mut(),
                loaded.root,
                leader_speed,
                defender,
                offender,
            )?;
            let geometry =
                emergent::Geometry::from(emergent::Primitive::cube(0.45, emergent::Color::CYAN)?);
            for id in [scene.leader, scene.follower, scene.tail] {
                loaded.renderables.push((id, geometry.clone()));
            }
            Some(scene)
        } else {
            None
        };
        let avoidance = scene
            .map(|scene| DistanceAvoidance::new(scene.defender, scene.offender))
            .or_else(|| {
                authored_pair.map(|(defender, offender)| DistanceAvoidance::new(defender, offender))
            });
        let bounce = loaded
            .entities
            .get(OFFENDER_BOX_ID)
            .copied()
            .zip(loaded.entities.get(GROUND_ID).copied())
            .map(|(offender, ground)| GroundBounce::new(offender, ground, 0.5));
        let jumping_bean = loaded
            .entities
            .get(FALLING_CUBE_ID)
            .copied()
            .filter(|entity| {
                engine
                    .world()
                    .component::<emergent::PhysicsBody>(*entity)
                    .ok()
                    .flatten()
                    .is_some_and(|body| body.body == emergent::BodyKind::Dynamic)
            })
            .map(PeriodicPop::new);
        Ok(Self {
            loaded,
            scene,
            avoidance,
            bounce,
            jumping_bean,
            exit_after_ticks,
        })
    }
}

impl Game for FirstProjectGame {
    fn start(&mut self, _context: &mut EngineContext<'_>) {
        eprintln!("Project scene constructed from authored assets through public Emergent APIs");
    }

    fn fixed_update(&mut self, context: &mut EngineContext<'_>) {
        if let Some(jumping_bean) = &mut self.jumping_bean {
            jumping_bean
                .update(context.world, context.fixed_delta)
                .expect("authored falling-cube remains a dynamic physics body");
        }

        if let Some(bounce) = &mut self.bounce {
            bounce
                .update(context.world, context.fixed_delta)
                .expect("authored OffenderBox and ground remain valid");
        }

        if let Some(avoidance) = self.avoidance {
            avoidance
                .update(context.world)
                .expect("authored Defender and OffenderBox remain valid");
        }

        for (id, degrees_per_second) in [
            ("cube", SLOW_SPIN_DEGREES_PER_SECOND),
            (CENTER_CUBE_ID, CENTER_CUBE_SPIN_DEGREES_PER_SECOND),
        ] {
            if let Some(&cube) = self.loaded.entities.get(id).filter(|entity| {
                !context
                    .world
                    .component::<emergent::PhysicsBody>(**entity)
                    .ok()
                    .flatten()
                    .is_some_and(|p| p.body == emergent::BodyKind::Dynamic)
            }) {
                rotate_z(context.world, cube, degrees_per_second, context.fixed_delta)
                    .expect("authored cube remains alive with a valid transform");
            }
        }

        if self
            .exit_after_ticks
            .is_some_and(|limit| context.tick.saturating_add(1) >= limit)
        {
            context.request_exit();
        }
    }

    fn after_fixed_update(&mut self, context: &mut EngineContext<'_>) {
        if context.tick.is_multiple_of(60) {
            let Some(scene) = self.scene else { return };
            let leader = context
                .world
                .global_transform(scene.leader)
                .expect("leader remains alive")
                .translation();
            let tail = context
                .world
                .global_transform(scene.tail)
                .expect("tail remains alive")
                .translation();
            println!(
                "tick {:>3}: leader.x={:.2}, tail.x={:.2}",
                context.tick, leader.x, tail.x
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use emergent::{Engine, EngineConfig, Quat, Vec3};

    use super::FirstProjectGame;

    #[test]
    fn authored_physics_demo_falls_settles_and_reconstructs_without_saving() {
        let project = emergent::Project::load(env!("CARGO_MANIFEST_DIR")).unwrap();
        let path = "assets/scenes/physics.scene.toml";
        let mut engine = Engine::new(EngineConfig::default()).unwrap();
        let mut game = FirstProjectGame::new(&mut engine, &project, path, 0.0, None).unwrap();
        let cube = game.loaded.entities["falling-cube"];
        let authored = *engine.world().local_transform(cube).unwrap();
        engine.run_ticks(&mut game, 360).unwrap();
        let resting = engine
            .world()
            .global_transform(cube)
            .unwrap()
            .translation()
            .y;
        assert!((resting - 0.5).abs() < 0.03, "{resting}");
        let mut fresh = Engine::new(EngineConfig::default()).unwrap();
        let reset = FirstProjectGame::new(&mut fresh, &project, path, 0.0, None).unwrap();
        assert_eq!(
            *fresh
                .world()
                .local_transform(reset.loaded.entities["falling-cube"])
                .unwrap(),
            authored
        );
    }

    #[test]
    fn authored_falling_cube_periodically_pops_off_the_ground() {
        let config = EngineConfig {
            fixed_timestep: Duration::from_secs_f64(1.0 / 60.0),
            ..EngineConfig::default()
        };
        let project = emergent::Project::load(env!("CARGO_MANIFEST_DIR")).unwrap();
        let mut engine = Engine::new(config).unwrap();
        let mut game = FirstProjectGame::new(
            &mut engine,
            &project,
            "assets/scenes/physics.scene.toml",
            0.0,
            None,
        )
        .unwrap();
        let cube = game.loaded.entities[super::FALLING_CUBE_ID];

        engine.run_ticks(&mut game, 250).unwrap();

        let height = engine
            .world()
            .global_transform(cube)
            .unwrap()
            .translation()
            .y;
        assert!(height > 0.75, "jumping bean should be airborne, y={height}");
    }

    #[test]
    fn authored_cube_spins_fifteen_degrees_in_one_second() {
        let config = EngineConfig {
            fixed_timestep: Duration::from_secs_f64(1.0 / 60.0),
            ..EngineConfig::default()
        };
        let mut engine = Engine::new(config).unwrap();
        let project = emergent::Project::load(env!("CARGO_MANIFEST_DIR")).unwrap();
        let path = project.manifest().entry_scene.to_str().unwrap();
        let mut game = FirstProjectGame::new(&mut engine, &project, path, 0.75, None).unwrap();

        let cube = game.loaded.entities["cube"];
        let initial = *engine.world().local_transform(cube).unwrap();
        engine.run_ticks(&mut game, 60).unwrap();
        let transform = *engine.world().local_transform(cube).unwrap();
        let rotated_x = transform.rotation * Vec3::X;
        let expected = initial.rotation * Quat::from_rotation_z(15.0_f32.to_radians()) * Vec3::X;
        assert!(rotated_x.abs_diff_eq(expected, 1.0e-4));
        assert_eq!(transform.translation, initial.translation);
        assert_eq!(transform.scale, initial.scale);
    }

    #[test]
    fn center_cube_rule_spins_ninety_degrees_in_one_second() {
        let config = EngineConfig {
            fixed_timestep: Duration::from_secs_f64(1.0 / 60.0),
            ..EngineConfig::default()
        };
        let mut engine = Engine::new(config).unwrap();
        let project = emergent::Project::load(env!("CARGO_MANIFEST_DIR")).unwrap();
        let path = project.manifest().entry_scene.to_str().unwrap();
        let mut game = FirstProjectGame::new(&mut engine, &project, path, 0.75, None).unwrap();

        // The IDE scene is editable; give the rule an independent test instance.
        let cube = engine.world_mut().spawn("Centre cube test");
        engine
            .world_mut()
            .set_local_transform(
                cube,
                emergent::Transform::new(Vec3::ZERO, Quat::IDENTITY, Vec3::splat(2.0)),
            )
            .unwrap();
        game.loaded
            .entities
            .insert(super::CENTER_CUBE_ID.into(), cube);
        let initial = *engine.world().local_transform(cube).unwrap();
        assert_eq!(initial.translation, Vec3::ZERO);
        assert_eq!(initial.scale, Vec3::splat(2.0));

        engine.run_ticks(&mut game, 60).unwrap();
        let transform = *engine.world().local_transform(cube).unwrap();
        let rotated_x = transform.rotation * Vec3::X;
        let expected = initial.rotation * Quat::from_rotation_z(90.0_f32.to_radians()) * Vec3::X;
        assert!(rotated_x.abs_diff_eq(expected, 1.0e-4));
        assert_eq!(transform.translation, Vec3::ZERO);
        assert_eq!(transform.scale, Vec3::splat(2.0));
    }

    #[test]
    fn defender_keeps_two_world_units_from_offender_box() {
        let mut engine = Engine::new(EngineConfig::default()).unwrap();
        let project = emergent::Project::load(env!("CARGO_MANIFEST_DIR")).unwrap();
        let path = project.manifest().entry_scene.to_str().unwrap();
        let mut game = FirstProjectGame::new(&mut engine, &project, path, 0.75, None).unwrap();
        let defender = game.loaded.entities[super::DEFENDER_ID];
        let offender = game.loaded.entities[super::OFFENDER_BOX_ID];
        let initial = *engine.world().local_transform(defender).unwrap();

        engine.run_ticks(&mut game, 1).unwrap();

        let transform = *engine.world().local_transform(defender).unwrap();
        let defender_position = engine
            .world()
            .global_transform(defender)
            .unwrap()
            .translation();
        let offender_position = engine
            .world()
            .global_transform(offender)
            .unwrap()
            .translation();
        assert!(defender_position.distance(offender_position) >= 2.0 - 1.0e-5);
        assert_eq!(transform.rotation, initial.rotation);
        assert_eq!(transform.scale, initial.scale);
    }

    #[test]
    fn authored_offender_bounces_about_four_metres_above_ground() {
        let config = EngineConfig {
            fixed_timestep: Duration::from_secs_f64(1.0 / 60.0),
            ..EngineConfig::default()
        };
        let mut engine = Engine::new(config).unwrap();
        let project = emergent::Project::load(env!("CARGO_MANIFEST_DIR")).unwrap();
        let path = project.manifest().entry_scene.to_str().unwrap();
        let mut game = FirstProjectGame::new(&mut engine, &project, path, 0.75, None).unwrap();
        let offender = game.loaded.entities[super::OFFENDER_BOX_ID];
        let initial = *engine.world().local_transform(offender).unwrap();
        let ground_y = engine
            .world()
            .global_transform(game.loaded.entities[super::GROUND_ID])
            .unwrap()
            .translation()
            .y;

        let mut maximum = f32::NEG_INFINITY;
        for _ in 0..180 {
            engine.run_ticks(&mut game, 1).unwrap();
            maximum = maximum.max(
                engine
                    .world()
                    .global_transform(offender)
                    .unwrap()
                    .translation()
                    .y,
            );
        }

        let transform = *engine.world().local_transform(offender).unwrap();
        let contact_y = ground_y + 0.5;
        assert!(((maximum - contact_y) - 4.0).abs() < 0.2, "{maximum}");
        assert_eq!(transform.rotation, initial.rotation);
        assert_eq!(transform.scale, initial.scale);
    }
}
