//! Per-tick keyboard snapshot. Platforms report held keys; the engine derives
//! pressed/released edges once per fixed tick, so headless runs can script input
//! and replay it deterministically.
use bevy_ecs::resource::Resource;

/// Keys the engine reports. Games map them to actions.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[repr(u8)]
pub enum Key {
    W,
    A,
    S,
    D,
    Q,
    E,
    Space,
    Shift,
    Up,
    Down,
    Left,
    Right,
}

#[cfg(feature = "graphics")]
impl Key {
    /// Maps a physical key from the native window (layout independent: WASD stays in place).
    pub fn from_winit(code: winit::keyboard::KeyCode) -> Option<Self> {
        use winit::keyboard::KeyCode as K;
        Some(match code {
            K::KeyW => Self::W,
            K::KeyA => Self::A,
            K::KeyS => Self::S,
            K::KeyD => Self::D,
            K::KeyQ => Self::Q,
            K::KeyE => Self::E,
            K::Space => Self::Space,
            K::ShiftLeft | K::ShiftRight => Self::Shift,
            K::ArrowUp => Self::Up,
            K::ArrowDown => Self::Down,
            K::ArrowLeft => Self::Left,
            K::ArrowRight => Self::Right,
            _ => return None,
        })
    }
}

/// A fixed-size set of keys (no allocation).
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub struct KeySet(u32);

impl KeySet {
    pub const EMPTY: Self = Self(0);

    pub const fn with(self, key: Key) -> Self {
        Self(self.0 | 1 << key as u8)
    }

    pub fn insert(&mut self, key: Key) {
        *self = self.with(key);
    }

    pub fn remove(&mut self, key: Key) {
        self.0 &= !(1 << key as u8);
    }

    pub const fn contains(self, key: Key) -> bool {
        self.0 & 1 << key as u8 != 0
    }
}

/// Input for the current fixed tick, available as a world resource and in `EngineContext`.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Resource)]
pub struct Input {
    held: KeySet,
    previous: KeySet,
}

impl Input {
    /// The snapshot for the next tick, given what was held on the previous tick.
    pub(crate) fn advance(self, held: KeySet) -> Self {
        Self {
            held,
            previous: self.held,
        }
    }

    pub fn held(self, key: Key) -> bool {
        self.held.contains(key)
    }

    /// Went down since the previous tick.
    pub fn pressed(self, key: Key) -> bool {
        self.held.contains(key) && !self.previous.contains(key)
    }

    pub fn released(self, key: Key) -> bool {
        !self.held.contains(key) && self.previous.contains(key)
    }

    /// -1, 0 or 1 from a pair of keys.
    pub fn axis(self, negative: Key, positive: Key) -> f32 {
        f32::from(u8::from(self.held(positive))) - f32::from(u8::from(self.held(negative)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn edges_are_derived_once_per_tick() {
        let held = KeySet::EMPTY.with(Key::W).with(Key::Space);
        let first = Input::default().advance(held);
        assert!(first.pressed(Key::Space) && first.held(Key::W));
        let second = first.advance(held);
        assert!(!second.pressed(Key::Space) && second.held(Key::Space));
        let third = second.advance(KeySet::EMPTY.with(Key::W));
        assert!(third.released(Key::Space));
        assert_eq!(third.axis(Key::S, Key::W), 1.0);
        let mut set = held;
        set.remove(Key::W);
        assert!(!set.contains(Key::W) && set.contains(Key::Space));
    }
}

/// Relative device motion consumed once at a fixed-tick boundary. No time scaling.
#[derive(Clone, Copy, Debug, Default, PartialEq, Resource)]
pub struct MouseMotion {
    pub delta: crate::Vec2,
}
