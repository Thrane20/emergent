# Tween assets and editor

Schema-1 `.tween.toml` assets are reusable scalar animation recipes: easing,
duration in seconds, from/to values and once/loop/ping-pong playback. Runtime
`Tween` is a validated, immutable owned value with allocation-free sampling at
explicit elapsed simulation time. It has no entity ownership; a behaviour owns
its clock and writes through existing EntityId/World APIs. No competing identity,
automatic transform writers, or wall-clock scheduling enters the engine.

The IDE's `/tweens` editor compares linear, sine, quadratic and cubic curves,
animates a cube left/right, and saves the selected recipe with a UUID independent
of its display name. Browser animation uses requestAnimationFrame and does not
advance gameplay. Preview values are bounded; saving validates the same TOML
through a headless Rust tool and publishes through existing guarded file writes.
Existing tween assets can be reopened as variants; saving creates a new asset
without overwriting authored content. Attaching tweens to scene behaviours is a
separate slice. Unit checks compare browser sampling against native Rust samples,
cover endpoints, repeats, invalid inputs, save/reload and duplicate names; browser
checks cover selection, moving preview and saved variant recovery.
