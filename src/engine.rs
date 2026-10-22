use std::error::Error;
use std::fmt;
use std::thread;
use std::time::{Duration, Instant};

use crate::World;

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
}

impl fmt::Display for EngineError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ZeroFixedTimestep => formatter.write_str("fixed timestep must be non-zero"),
            Self::ZeroStepLimit => formatter.write_str("max steps per frame must be non-zero"),
        }
    }
}

impl Error for EngineError {}

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
    tick: u64,
    exit_requested: bool,
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
            tick: 0,
            exit_requested: false,
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

    /// Runs against wall-clock time until the game requests exit.
    pub fn run<G: Game>(&mut self, game: &mut G) {
        self.begin(game);

        let mut previous = Instant::now();
        let mut accumulator = Duration::ZERO;

        while !self.exit_requested {
            let now = Instant::now();
            let elapsed = now
                .duration_since(previous)
                .min(self.config.max_frame_delta);
            previous = now;
            accumulator += elapsed;

            let mut steps = 0;
            while accumulator >= self.config.fixed_timestep
                && steps < self.config.max_steps_per_frame
                && !self.exit_requested
            {
                self.update_once(game);
                accumulator -= self.config.fixed_timestep;
                steps += 1;
            }

            if steps == self.config.max_steps_per_frame {
                accumulator = accumulator.min(self.config.fixed_timestep);
            }

            if !self.exit_requested && accumulator < self.config.fixed_timestep {
                let until_next_step = self.config.fixed_timestep - accumulator;
                thread::sleep(until_next_step.min(self.config.idle_sleep));
            }
        }

        self.end(game);
    }

    /// Advances an exact number of ticks without sleeping or reading the clock.
    pub fn run_ticks<G: Game>(&mut self, game: &mut G, ticks: u64) {
        self.begin(game);
        for _ in 0..ticks {
            if self.exit_requested {
                break;
            }
            self.update_once(game);
        }
        self.end(game);
    }

    fn begin<G: Game>(&mut self, game: &mut G) {
        self.exit_requested = false;
        {
            let mut context = self.context();
            game.start(&mut context);
        }
        self.world.propagate_transforms();
    }

    fn update_once<G: Game>(&mut self, game: &mut G) {
        {
            let mut context = self.context();
            game.fixed_update(&mut context);
        }
        self.world.propagate_transforms();
        {
            let mut context = self.context();
            game.after_fixed_update(&mut context);
        }
        self.tick = self.tick.saturating_add(1);
    }

    fn end<G: Game>(&mut self, game: &mut G) {
        let mut context = self.context();
        game.stop(&mut context);
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
    use super::*;

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

        engine.run_ticks(&mut game, 10);

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
}
