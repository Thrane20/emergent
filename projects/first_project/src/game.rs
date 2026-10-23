use emergent::{EngineContext, Game};

use crate::generated::main_scene::{build_main_scene, MainScene};

pub struct FirstProjectGame {
    leader_speed: f32,
    scene: Option<MainScene>,
    exit_after_ticks: Option<u64>,
}

impl FirstProjectGame {
    pub fn new(leader_speed: f32, exit_after_ticks: Option<u64>) -> Self {
        Self {
            leader_speed,
            scene: None,
            exit_after_ticks,
        }
    }

    pub fn scene(&self) -> Option<MainScene> {
        self.scene
    }
}

impl Game for FirstProjectGame {
    fn start(&mut self, context: &mut EngineContext<'_>) {
        self.scene = Some(
            build_main_scene(context.world, self.leader_speed)
                .expect("validated generated scene must build"),
        );
        println!("Project scene constructed through public Emergent APIs");
    }

    fn fixed_update(&mut self, context: &mut EngineContext<'_>) {
        if self
            .exit_after_ticks
            .is_some_and(|limit| context.tick.saturating_add(1) >= limit)
        {
            context.request_exit();
        }
    }

    fn after_fixed_update(&mut self, context: &mut EngineContext<'_>) {
        if context.tick.is_multiple_of(60) {
            let scene = self.scene.expect("project scene is constructed");
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
            let key_light = context
                .world
                .global_transform(scene.key_light)
                .expect("key light remains alive")
                .translation();
            println!(
                "tick {:>3}: leader.x={:.2}, tail.x={:.2}, key_light=({:.1}, {:.1}, {:.1})",
                context.tick, leader.x, tail.x, key_light.x, key_light.y, key_light.z
            );
        }
    }
}
