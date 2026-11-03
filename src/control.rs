//! Fixed-step feedback control, independent of entities and physics.
//!
//! A caller owns one [`Pid`] per controlled scalar. Gains use seconds, not update
//! counts: I integrates error over `dt`; D differentiates the measurement. Feedforward
//! and actuator allocation belong to the caller. There is no allocation in stepping.
use std::{fmt, time::Duration};

/// PID configuration. Gains assume increasing output increases the measured value.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PidConfig {
    /// Output units per error unit.
    pub kp: f32,
    /// Output units per error unit per second.
    pub ki: f32,
    /// Output units times seconds per measurement unit.
    pub kd: f32,
    /// Absolute bound on the integral contribution, in output units.
    pub integral_limit: f32,
    pub output_min: f32,
    pub output_max: f32,
    /// First-order derivative low-pass time constant; zero disables filtering.
    pub derivative_filter_seconds: f32,
    /// Back-calculation rate (1/s) for previous requested versus achieved output.
    /// Zero disables tracking; conditional integration still handles internal clipping.
    pub tracking_gain: f32,
}

impl Default for PidConfig {
    fn default() -> Self {
        Self {
            kp: 1.0,
            ki: 0.0,
            kd: 0.0,
            integral_limit: 1.0,
            output_min: -1.0,
            output_max: 1.0,
            derivative_filter_seconds: 0.0,
            tracking_gain: 1.0,
        }
    }
}

/// Current observation and setpoint, with optional feedback from the last interval.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PidInput {
    pub setpoint: f32,
    pub measurement: f32,
    /// Added in output units before output limiting.
    pub feedforward: f32,
    /// Measured derivative in units/s. Otherwise inferred from consecutive samples.
    pub measurement_rate: Option<f32>,
    /// Total achieved output (including feedforward), in this PID's output units,
    /// during the preceding interval. Use the previous command's actuator result,
    /// not the current desired command. None tracks internal clipping only.
    pub achieved_output: Option<f32>,
}

/// Contributions and command for plotting and actuator submission.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PidOutput {
    pub proportional: f32,
    pub integral: f32,
    pub derivative: f32,
    /// Sum of P/I/D and feedforward, before output limiting.
    pub requested: f32,
    pub output: f32,
    /// Internal output clipping; downstream actuator limits are reported by its caller.
    pub saturated: bool,
    /// Previous achieved minus requested output; zero on the first sample.
    pub tracking_error: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ControlError {
    InvalidConfig,
    InvalidInput,
    NonFiniteOutput,
}
impl fmt::Display for ControlError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::InvalidConfig => "PID requires finite nonnegative gains and integral/filter limits, and finite ordered output bounds",
            Self::InvalidInput => "PID requires finite observations and a positive timestep",
            Self::NonFiniteOutput => "PID arithmetic exceeded its finite range; controller state was preserved",
        })
    }
}
impl std::error::Error for ControlError {}

/// Validated configuration and private, per-controller history.
///
/// Stop calling `step` while disabled/paused. Call [`Self::reset`] on re-enable,
/// body reconstruction or teleport; pass only simulation time, never paused wall
/// time. Changing gains constructs a new controller and resets history. Repeated
/// setpoints preserve history. A failed step leaves all history unchanged.
#[derive(Clone, Debug, PartialEq)]
pub struct Pid {
    config: PidConfig,
    integral: f32,
    previous_measurement: Option<f32>,
    filtered_rate: f32,
    previous_command: Option<(f32, f32)>,
}

impl Pid {
    pub fn new(config: PidConfig) -> Result<Self, ControlError> {
        if [
            config.kp,
            config.ki,
            config.kd,
            config.integral_limit,
            config.derivative_filter_seconds,
            config.tracking_gain,
        ]
        .iter()
        .any(|v| !v.is_finite() || *v < 0.0)
            || !config.output_min.is_finite()
            || !config.output_max.is_finite()
            || config.output_min > config.output_max
        {
            return Err(ControlError::InvalidConfig);
        }
        Ok(Self {
            config,
            integral: 0.0,
            previous_measurement: None,
            filtered_rate: 0.0,
            previous_command: None,
        })
    }

    pub fn reset(&mut self) {
        self.integral = 0.0;
        self.previous_measurement = None;
        self.filtered_rate = 0.0;
        self.previous_command = None;
    }

    /// Advance once over the elapsed simulation interval. The first inferred
    /// derivative is zero; a supplied derivative is filtered from zero initially.
    /// Tracking gains are scaled with a backward-Euler factor to keep recovery
    /// bounded even when the configured tracking time is shorter than one step.
    pub fn step(&mut self, input: PidInput, dt: Duration) -> Result<PidOutput, ControlError> {
        let dt = dt.as_secs_f32();
        if !dt.is_finite()
            || dt <= 0.0
            || [input.setpoint, input.measurement, input.feedforward]
                .iter()
                .any(|v| !v.is_finite())
            || input.measurement_rate.is_some_and(|v| !v.is_finite())
            || input.achieved_output.is_some_and(|v| !v.is_finite())
        {
            return Err(ControlError::InvalidInput);
        }
        let c = self.config;
        let error = input.setpoint - input.measurement;
        let proportional = c.kp * error;
        let rate = input.measurement_rate.unwrap_or_else(|| {
            self.previous_measurement
                .map_or(0.0, |previous| (input.measurement - previous) / dt)
        });
        let alpha = dt / (c.derivative_filter_seconds + dt);
        let filtered_rate = self.filtered_rate + alpha * (rate - self.filtered_rate);
        let derivative = -c.kd * filtered_rate;
        let tracking_error = self.previous_command.map_or(0.0, |(requested, output)| {
            input.achieved_output.unwrap_or(output) - requested
        });
        let tracking_step = c.tracking_gain as f64 * dt as f64;
        let tracking_factor = tracking_step / (1.0 + tracking_step);
        let tracked = self.integral + tracking_factor as f32 * tracking_error;
        let base_integral = tracked.clamp(-c.integral_limit, c.integral_limit);
        let increment = c.ki * error * dt;
        let integrated = base_integral + increment;
        let mut integral = if c.ki == 0.0 {
            0.0
        } else {
            integrated.clamp(-c.integral_limit, c.integral_limit)
        };
        let request = |i| proportional + i + derivative + input.feedforward;
        let initial_request = request(integral);
        // Only accumulate error when it does not push further into output saturation.
        if c.ki != 0.0
            && (initial_request > c.output_max && increment > 0.0
                || initial_request < c.output_min && increment < 0.0)
        {
            integral = base_integral;
        }
        let requested = request(integral);
        if [
            error,
            proportional,
            rate,
            filtered_rate,
            derivative,
            tracking_error,
            tracked,
            increment,
            integrated,
            initial_request,
            requested,
        ]
        .iter()
        .any(|v| !v.is_finite())
        {
            return Err(ControlError::NonFiniteOutput);
        }
        let output = requested.clamp(c.output_min, c.output_max);
        self.integral = integral;
        self.previous_measurement = Some(input.measurement);
        self.filtered_rate = filtered_rate;
        self.previous_command = Some((requested, output));
        Ok(PidOutput {
            proportional,
            integral,
            derivative,
            requested,
            output,
            saturated: output != requested,
            tracking_error,
        })
    }
}
