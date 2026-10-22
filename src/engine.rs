use std::error::Error;
use std::fmt;
use std::thread;
use std::time::{Duration, Instant};

use bevy_ecs::resource::Resource;
use bevy_ecs::schedule::Schedule;

use crate::{World, WorldError};

/// Configuration for the fixed-step engine loop.
#[derive(Clone, Copy, Debug)]
pub struct EngineConfig {
    pub fixed_timestep: Duration,
    pub max_frame_delta: Duration,
    pub max_steps_per_frame: u32,
    pub idle_sleep: Duration,
}

impl Default for EngineConfig {
    fn default() -> Self {
        Self {
            fixed_timestep: Duration::from_secs_f64(1.0 / 60.0),
            max_frame_delta: Duration::from_millis(250),
            max_steps_per_frame: 8,
            idle_sleep: Duration::from_millis(1),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EngineError {
    ZeroFixedTimestep,
    ZeroStepLimit,
    AlreadyStarted,
    NotStarted,
    World(WorldError),
}

impl fmt::Display for EngineError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ZeroFixedTimestep => formatter.write_str("fixed timestep must be non-zero"),
            Self::ZeroStepLimit => formatter.write_str("max steps per frame must be non-zero"),
            Self::AlreadyStarted => formatter.write_str("engine is already started"),
            Self::NotStarted => formatter.write_str("engine has not been started"),
            Self::World(error) => write!(formatter, "world integrity error: {error}"),
        }
    }
}

impl Error for EngineError {}

impl From<WorldError> for EngineError {
    fn from(error: WorldError) -> Self {
        Self::World(error)
    }
}

/// Fixed-tick schedule positions owned by the engine.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SchedulePhase {
    PreFixed,
    Fixed,
    PostFixed,
}

/// Current fixed-step timing exposed to Bevy systems.
#[derive(Clone, Copy, Debug, Resource)]
pub struct SimulationTime {
    pub tick: u64,
    pub fixed_delta: Duration,
}

/// Result of advancing a running engine by one render-frame duration.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct FrameAdvance {
    pub fixed_steps: u32,
    pub catch_up_limited: bool,
}

#[derive(Default)]
struct EngineSchedules {
    pre_fixed: Schedule,
    fixed: Schedule,
    post_fixed: Schedule,
}

impl EngineSchedules {
    fn get_mut(&mut self, phase: SchedulePhase) -> &mut Schedule {
        match phase {
            SchedulePhase::PreFixed => &mut self.pre_fixed,
            SchedulePhase::Fixed => &mut self.fixed,
            SchedulePhase::PostFixed => &mut self.post_fixed,
        }
    }
}

/// The interface implemented by a game using the Emergent loop.
pub trait Game {
    fn start(&mut self, _context: &mut EngineContext<'_>) {}

    fn fixed_update(&mut self, context: &mut EngineContext<'_>);

    /// Runs after the engine has propagated transforms for the current tick.
    fn after_fixed_update(&mut self, _context: &mut EngineContext<'_>) {}

    fn stop(&mut self, _context: &mut EngineContext<'_>) {}
}

/// State made available during a game callback.
pub struct EngineContext<'a> {
    pub world: &'a mut World,
    pub tick: u64,
    pub fixed_delta: Duration,
    exit_requested: &'a mut bool,
}

impl EngineContext<'_> {
    pub fn request_exit(&mut self) {
        *self.exit_requested = true;
    }

    pub fn exit_requested(&self) -> bool {
        *self.exit_requested
    }
}

/// Owns the runtime world and advances it at a fixed simulation rate.
pub struct Engine {
    config: EngineConfig,
    world: World,
    schedules: EngineSchedules,
    tick: u64,
    exit_requested: bool,
    started: bool,
    accumulator: Duration,
}

impl Engine {
    pub fn new(config: EngineConfig) -> Result<Self, EngineError> {
        if config.fixed_timestep.is_zero() {
            return Err(EngineError::ZeroFixedTimestep);
        }
        if config.max_steps_per_frame == 0 {
            return Err(EngineError::ZeroStepLimit);
        }

        Ok(Self {
            config,
            world: World::new(),
            schedules: EngineSchedules::default(),
            tick: 0,
            exit_requested: false,
            started: false,
            accumulator: Duration::ZERO,
        })
    }

    pub fn world(&self) -> &World {
        &self.world
    }

    pub fn world_mut(&mut self) -> &mut World {
        &mut self.world
    }

    pub fn tick(&self) -> u64 {
        self.tick
    }

    pub fn schedule_mut(&mut self, phase: SchedulePhase) -> &mut Schedule {
        self.schedules.get_mut(phase)
    }

    pub fn exit_requested(&self) -> bool {
        self.exit_requested
    }

    /// Starts a game so an external platform loop can advance it frame by frame.
    pub fn start<G: Game>(&mut self, game: &mut G) -> Result<(), EngineError> {
        if self.started {
            return Err(EngineError::AlreadyStarted);
        }
        self.exit_requested = false;
        self.accumulator = Duration::ZERO;
        {
            let mut context = self.context();
            game.start(&mut context);
        }
        self.world.propagate_transforms();
        self.world.validate_component_mappings()?;
        self.started = true;
        Ok(())
    }

    /// Advances fixed simulation using elapsed time supplied by a platform loop.
    pub fn advance_frame<G: Game>(
        &mut self,
        game: &mut G,
        elapsed: Duration,
    ) -> Result<FrameAdvance, EngineError> {
        if !self.started {
            return Err(EngineError::NotStarted);
        }

        self.accumulator += elapsed.min(self.config.max_frame_delta);
        let mut fixed_steps = 0;
        while self.accumulator >= self.config.fixed_timestep
            && fixed_steps < self.config.max_steps_per_frame
            && !self.exit_requested
        {
            self.update_once(game)?;
            self.accumulator -= self.config.fixed_timestep;
            fixed_steps += 1;
        }

        let catch_up_limited = fixed_steps == self.config.max_steps_per_frame
            && self.accumulator >= self.config.fixed_timestep;
        if catch_up_limited {
            self.accumulator = self.accumulator.min(self.config.fixed_timestep);
        }

        Ok(FrameAdvance {
            fixed_steps,
            catch_up_limited,
        })
    }

    /// Stops a game previously started through either engine loop.
    pub fn stop<G: Game>(&mut self, game: &mut G) {
        if self.started {
            let mut context = self.context();
            game.stop(&mut context);
            self.started = false;
            self.accumulator = Duration::ZERO;
        }
    }

    /// Runs against wall-clock time until the game requests exit.
    pub fn run<G: Game>(&mut self, game: &mut G) -> Result<(), EngineError> {
        self.start(game)?;

        let mut previous = Instant::now();

        while !self.exit_requested {
            let now = Instant::now();
            let elapsed = now
                .duration_since(previous)
                .min(self.config.max_frame_delta);
            previous = now;
            if let Err(error) = self.advance_frame(game, elapsed) {
                self.stop(game);
                return Err(error);
            }

            if !self.exit_requested && self.accumulator < self.config.fixed_timestep {
                let until_next_step = self.config.fixed_timestep - self.accumulator;
                thread::sleep(until_next_step.min(self.config.idle_sleep));
            }
        }

        self.stop(game);
        Ok(())
    }

    /// Advances an exact number of ticks without sleeping or reading the clock.
    pub fn run_ticks<G: Game>(&mut self, game: &mut G, ticks: u64) -> Result<(), EngineError> {
        self.start(game)?;
        for _ in 0..ticks {
            if self.exit_requested {
                break;
            }
            if let Err(error) = self.update_once(game) {
                self.stop(game);
                return Err(error);
            }
        }
        self.stop(game);
        Ok(())
    }

    fn update_once<G: Game>(&mut self, game: &mut G) -> Result<(), EngineError> {
        self.world.insert_resource(SimulationTime {
            tick: self.tick,
            fixed_delta: self.config.fixed_timestep,
        });
        self.world
            .run_component_schedule(&mut self.schedules.pre_fixed)?;
        {
            let mut context = self.context();
            game.fixed_update(&mut context);
        }
        self.world
            .run_component_schedule(&mut self.schedules.fixed)?;
        self.world.evaluate_behaviors(self.config.fixed_timestep)?;
        self.world.commit_lifecycle_commands();
        self.world.propagate_transforms();
        self.world
            .run_component_schedule(&mut self.schedules.post_fixed)?;
        {
            let mut context = self.context();
            game.after_fixed_update(&mut context);
        }
        self.tick = self.tick.saturating_add(1);
        Ok(())
    }

    fn context(&mut self) -> EngineContext<'_> {
        EngineContext {
            world: &mut self.world,
            tick: self.tick,
            fixed_delta: self.config.fixed_timestep,
            exit_requested: &mut self.exit_requested,
        }
    }
}

#[cfg(test)]
mod tests {
    use bevy_ecs::prelude::{
        Commands, Component, Entity as EcsEntity, Query, Res, ResMut, Resource, With,
    };

    use super::*;
    use crate::EntityId;

    #[derive(Default)]
    struct CounterGame {
        starts: u32,
        updates: u32,
        after_updates: u32,
        stops: u32,
    }

    impl Game for CounterGame {
        fn start(&mut self, _context: &mut EngineContext<'_>) {
            self.starts += 1;
        }

        fn fixed_update(&mut self, context: &mut EngineContext<'_>) {
            self.updates += 1;
            if self.updates == 3 {
                context.request_exit();
            }
        }

        fn after_fixed_update(&mut self, _context: &mut EngineContext<'_>) {
            self.after_updates += 1;
        }

        fn stop(&mut self, _context: &mut EngineContext<'_>) {
            self.stops += 1;
        }
    }

    #[test]
    fn exact_tick_run_honours_exit_and_lifecycle() {
        let mut engine = Engine::new(EngineConfig::default()).unwrap();
        let mut game = CounterGame::default();

        engine.run_ticks(&mut game, 10).unwrap();

        assert_eq!(game.starts, 1);
        assert_eq!(game.updates, 3);
        assert_eq!(game.after_updates, 3);
        assert_eq!(game.stops, 1);
        assert_eq!(engine.tick(), 3);
    }

    #[test]
    fn rejects_invalid_loop_configuration() {
        let zero_timestep = EngineConfig {
            fixed_timestep: Duration::ZERO,
            ..EngineConfig::default()
        };
        assert!(matches!(
            Engine::new(zero_timestep),
            Err(EngineError::ZeroFixedTimestep)
        ));

        let zero_steps = EngineConfig {
            max_steps_per_frame: 0,
            ..EngineConfig::default()
        };
        assert!(matches!(
            Engine::new(zero_steps),
            Err(EngineError::ZeroStepLimit)
        ));
    }

    #[test]
    fn external_frame_loop_uses_bounded_fixed_catch_up() {
        let config = EngineConfig {
            fixed_timestep: Duration::from_millis(10),
            max_steps_per_frame: 2,
            ..EngineConfig::default()
        };
        let mut engine = Engine::new(config).unwrap();
        let mut game = CounterGame::default();

        assert_eq!(
            engine.advance_frame(&mut game, Duration::ZERO),
            Err(EngineError::NotStarted)
        );
        engine.start(&mut game).unwrap();
        assert_eq!(engine.start(&mut game), Err(EngineError::AlreadyStarted));
        assert_eq!(
            engine
                .advance_frame(&mut game, Duration::from_millis(35))
                .unwrap(),
            FrameAdvance {
                fixed_steps: 2,
                catch_up_limited: true,
            }
        );
        assert_eq!(
            engine.advance_frame(&mut game, Duration::ZERO).unwrap(),
            FrameAdvance {
                fixed_steps: 1,
                catch_up_limited: false,
            }
        );
        assert!(engine.exit_requested());
        engine.stop(&mut game);

        assert_eq!((game.starts, game.updates, game.stops), (1, 3, 1));
    }

    #[derive(Component)]
    struct Position(f32);

    #[derive(Component)]
    struct Velocity(f32);

    #[derive(Default, Resource)]
    struct PhaseLog(Vec<&'static str>);

    fn prepare(mut log: ResMut<PhaseLog>, time: Res<SimulationTime>) {
        assert_eq!(time.tick, 0);
        assert_eq!(time.fixed_delta, Duration::from_millis(10));
        log.0.push("pre");
    }

    fn simulate(
        mut bodies: Query<(&mut Position, &Velocity)>,
        mut queue: ResMut<crate::EntityLifecycleQueue>,
        mut log: ResMut<PhaseLog>,
    ) {
        for (mut position, velocity) in &mut bodies {
            position.0 += velocity.0;
        }
        queue.spawn("scheduled spawn");
        log.0.push("fixed");
    }

    fn observe(results: Res<crate::EntityLifecycleResults>, mut log: ResMut<PhaseLog>) {
        assert!(matches!(
            results.as_slice(),
            [crate::EntityLifecycleResult::Spawned(_)]
        ));
        log.0.push("post");
    }

    struct ScheduledGame {
        entity: EntityId,
    }

    impl Game for ScheduledGame {
        fn fixed_update(&mut self, context: &mut EngineContext<'_>) {
            context
                .world
                .resource_mut::<PhaseLog>()
                .unwrap()
                .0
                .push("game");
        }

        fn after_fixed_update(&mut self, context: &mut EngineContext<'_>) {
            assert_eq!(
                context
                    .world
                    .component::<Position>(self.entity)
                    .unwrap()
                    .unwrap()
                    .0,
                3.0
            );
            context
                .world
                .resource_mut::<PhaseLog>()
                .unwrap()
                .0
                .push("after");
        }
    }

    #[test]
    fn runs_schedules_and_commits_lifecycle_between_fixed_and_post_fixed() {
        let config = EngineConfig {
            fixed_timestep: Duration::from_millis(10),
            ..EngineConfig::default()
        };
        let mut engine = Engine::new(config).unwrap();
        let entity = engine.world_mut().spawn("body");
        engine
            .world_mut()
            .insert_component(entity, Position(1.0))
            .unwrap();
        engine
            .world_mut()
            .insert_component(entity, Velocity(2.0))
            .unwrap();
        engine.world_mut().insert_resource(PhaseLog::default());
        engine
            .schedule_mut(SchedulePhase::PreFixed)
            .add_systems(prepare);
        engine
            .schedule_mut(SchedulePhase::Fixed)
            .add_systems(simulate);
        engine
            .schedule_mut(SchedulePhase::PostFixed)
            .add_systems(observe);

        engine.run_ticks(&mut ScheduledGame { entity }, 1).unwrap();

        assert_eq!(engine.world().len(), 2);
        assert_eq!(
            engine.world().resource::<PhaseLog>().unwrap().0,
            ["pre", "game", "fixed", "post", "after"]
        );
        engine.world().validate_component_mappings().unwrap();
    }

    fn unsupported_direct_spawn(mut commands: Commands) {
        commands.spawn_empty();
    }

    #[derive(Default)]
    struct EmptyGame;

    impl Game for EmptyGame {
        fn fixed_update(&mut self, _context: &mut EngineContext<'_>) {}
    }

    #[test]
    fn rejects_direct_ecs_entity_lifecycle_changes() {
        let mut engine = Engine::new(EngineConfig::default()).unwrap();
        engine
            .schedule_mut(SchedulePhase::Fixed)
            .add_systems(unsupported_direct_spawn);

        let error = engine.run_ticks(&mut EmptyGame, 1).unwrap_err();
        assert_eq!(
            error,
            EngineError::World(WorldError::ComponentStoreCardinality)
        );
    }

    fn unsupported_identity_removal(
        mut commands: Commands,
        identities: Query<EcsEntity, With<crate::EntityIdentity>>,
    ) {
        for entity in &identities {
            commands.entity(entity).remove::<crate::EntityIdentity>();
        }
    }

    #[test]
    fn rejects_removing_engine_owned_identity_in_a_schedule() {
        let mut engine = Engine::new(EngineConfig::default()).unwrap();
        engine.world_mut().spawn("canonical");
        engine
            .schedule_mut(SchedulePhase::Fixed)
            .add_systems(unsupported_identity_removal);

        let error = engine.run_ticks(&mut EmptyGame, 1).unwrap_err();
        assert_eq!(
            error,
            EngineError::World(WorldError::UnsupportedComponentLifecycle)
        );
    }

    fn unsupported_engine_resource_removal(mut commands: Commands) {
        commands.remove_resource::<crate::EntityLifecycleQueue>();
    }

    #[test]
    fn rejects_removing_engine_owned_resources_in_a_schedule() {
        let mut engine = Engine::new(EngineConfig::default()).unwrap();
        engine
            .schedule_mut(SchedulePhase::PreFixed)
            .add_systems(unsupported_engine_resource_removal);

        let error = engine.run_ticks(&mut EmptyGame, 1).unwrap_err();
        assert_eq!(error, EngineError::World(WorldError::MissingEngineResource));
    }
}
