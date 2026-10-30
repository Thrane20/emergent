//! Reusable scalar tweens sampled at explicit simulation time.
use crate::Project;
use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum Easing {
    Linear,
    SineIn,
    SineOut,
    SineInOut,
    QuadIn,
    QuadOut,
    QuadInOut,
    CubicIn,
    CubicOut,
    CubicInOut,
}

impl Easing {
    /// Maps finite progress clamped to [0, 1] to eased progress.
    pub fn sample(self, progress: f32) -> f32 {
        let t = progress.clamp(0., 1.);
        if t == 0. || t == 1. {
            return t;
        }
        match self {
            Self::Linear => t,
            Self::SineIn => 1. - (t * std::f32::consts::FRAC_PI_2).cos(),
            Self::SineOut => (t * std::f32::consts::FRAC_PI_2).sin(),
            Self::SineInOut => (1. - (t * std::f32::consts::PI).cos()) * 0.5,
            Self::QuadIn => t * t,
            Self::QuadOut => 1. - (1. - t).powi(2),
            Self::QuadInOut if t < 0.5 => 2. * t * t,
            Self::QuadInOut => 1. - (-2. * t + 2.).powi(2) * 0.5,
            Self::CubicIn => t * t * t,
            Self::CubicOut => 1. - (1. - t).powi(3),
            Self::CubicInOut if t < 0.5 => 4. * t * t * t,
            Self::CubicInOut => 1. - (-2. * t + 2.).powi(3) * 0.5,
        }
    }
}

/// Linear interpolation; progress is not clamped, allowing extrapolation.
pub fn lerp(from: f32, to: f32, progress: f32) -> f32 {
    from * (1. - progress) + to * progress
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum TweenPlayback {
    Once,
    Loop,
    PingPong,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct TweenSettings {
    pub easing: Easing,
    pub duration_seconds: f32,
    pub from: f32,
    pub to: f32,
    pub playback: TweenPlayback,
}

#[derive(Clone, Copy, Debug)]
pub struct Tween {
    settings: TweenSettings,
}
impl Tween {
    pub fn new(settings: TweenSettings) -> Result<Self, TweenError> {
        if !settings.duration_seconds.is_finite()
            || !(0.001..=3600.).contains(&settings.duration_seconds)
            || ![settings.from, settings.to]
                .iter()
                .all(|v| v.is_finite() && v.abs() <= 1_000_000.)
        {
            return Err(TweenError(
                "duration must be 0.001..3600 seconds; endpoints must be finite within +/-1000000"
                    .into(),
            ));
        }
        Ok(Self { settings })
    }
    pub fn settings(self) -> TweenSettings {
        self.settings
    }
    /// Negative time holds the start. Duration is one leg of a ping-pong cycle.
    pub fn sample(self, elapsed_seconds: f64) -> Result<f32, TweenError> {
        if !elapsed_seconds.is_finite() {
            return Err(TweenError("elapsed time must be finite".into()));
        }
        let duration = self.settings.duration_seconds as f64;
        let elapsed = elapsed_seconds.max(0.);
        let progress = match self.settings.playback {
            TweenPlayback::Once => (elapsed / duration).min(1.),
            TweenPlayback::Loop => (elapsed % duration) / duration,
            TweenPlayback::PingPong => {
                let phase = (elapsed % (2. * duration)) / duration;
                if phase <= 1. {
                    phase
                } else {
                    2. - phase
                }
            }
        } as f32;
        Ok(lerp(
            self.settings.from,
            self.settings.to,
            self.settings.easing.sample(progress),
        ))
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TweenDefinition {
    pub schema_version: u32,
    pub id: String,
    pub name: String,
    pub kind: String,
    pub tween: TweenSettings,
}
impl TweenDefinition {
    pub fn from_source(source: &str) -> Result<Self, TweenError> {
        if source.len() > 8192 {
            return Err(TweenError("tween asset exceeds 8 KiB".into()));
        }
        let definition: Self = toml::from_str(source).map_err(|e| TweenError(e.to_string()))?;
        if definition.schema_version != 1
            || definition.kind != "tween"
            || definition.id.is_empty()
            || definition.id.len() > 128
            || !definition
                .id
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || b"_.-".contains(&c))
            || definition.name.trim().is_empty()
            || definition.name.chars().count() > 64
            || definition.name.chars().any(char::is_control)
        {
            return Err(TweenError("invalid schema-1 tween identity or name".into()));
        }
        Tween::new(definition.tween)?;
        Ok(definition)
    }
    pub fn load(project: &Project, path: &str) -> Result<Self, TweenError> {
        if !path.ends_with(".tween.toml") {
            return Err(TweenError("choose a .tween.toml asset".into()));
        }
        let source = project
            .read_text_asset(path)
            .map_err(|e| TweenError(e.to_string()))?;
        Self::from_source(&source)
    }
    pub fn sampler(&self) -> Result<Tween, TweenError> {
        Tween::new(self.tween)
    }
}

#[derive(Debug)]
pub struct TweenError(pub String);
impl fmt::Display for TweenError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
impl std::error::Error for TweenError {}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn curves_are_bounded_monotonic_and_have_exact_endpoints() {
        for easing in [
            Easing::Linear,
            Easing::SineIn,
            Easing::SineOut,
            Easing::SineInOut,
            Easing::QuadIn,
            Easing::QuadOut,
            Easing::QuadInOut,
            Easing::CubicIn,
            Easing::CubicOut,
            Easing::CubicInOut,
        ] {
            assert_eq!(easing.sample(0.), 0.);
            assert_eq!(easing.sample(1.), 1.);
            let mut previous = 0.;
            for i in 0..=100 {
                let value = easing.sample(i as f32 / 100.);
                assert!((previous..=1.).contains(&value));
                previous = value;
            }
        }
        assert_eq!(lerp(2., 6., 0.25), 3.);
        assert!((Easing::SineInOut.sample(0.5) - 0.5).abs() < 1e-6);
    }
    #[test]
    fn time_boundaries_repeats_and_validation() {
        let settings = TweenSettings {
            easing: Easing::Linear,
            duration_seconds: 2.,
            from: -3.,
            to: 3.,
            playback: TweenPlayback::PingPong,
        };
        let tween = Tween::new(settings).unwrap();
        for (time, value) in [
            (-1., -3.),
            (0., -3.),
            (1., 0.),
            (2., 3.),
            (3., 0.),
            (4., -3.),
        ] {
            assert_eq!(tween.sample(time).unwrap(), value);
        }
        assert_eq!(
            Tween::new(TweenSettings {
                playback: TweenPlayback::Once,
                ..settings
            })
            .unwrap()
            .sample(20.)
            .unwrap(),
            3.
        );
        assert_eq!(
            Tween::new(TweenSettings {
                playback: TweenPlayback::Loop,
                ..settings
            })
            .unwrap()
            .sample(2.)
            .unwrap(),
            -3.
        );
        assert!(tween.sample(f64::INFINITY).is_err());
        assert!(tween.sample(f64::MAX).unwrap().is_finite());
        for duration_seconds in [0., -1., f32::NAN, 3601.] {
            assert!(Tween::new(TweenSettings {
                duration_seconds,
                ..settings
            })
            .is_err());
        }
        assert!(Tween::new(TweenSettings {
            from: f32::INFINITY,
            ..settings
        })
        .is_err());
        let source = toml::to_string(&TweenDefinition {
            schema_version: 1,
            id: "test".into(),
            name: "Test".into(),
            kind: "tween".into(),
            tween: settings,
        })
        .unwrap();
        assert!(TweenDefinition::from_source(&source).is_ok());
        assert!(TweenDefinition::from_source(
            &source.replace("schema_version = 1", "schema_version = 2")
        )
        .is_err());
        assert!(TweenDefinition::from_source(&(source + "\n[tween.extra]\nx=1\n")).is_err());
    }
}
