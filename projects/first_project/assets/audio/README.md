# Audio

Clips are `<name>.audio.toml` files beside their source audio (`.wav`, `.mp3`, `.ogg`
or `.flac`, mono or stereo). Play one from a scene object with the
`emergent.audio_source` component; the active camera (or an `emergent.audio_listener`)
hears it. See `docs/plans/audio.md` in the engine.

`scenes/audio-demo.scene.toml` orbits a humming cube past the camera (Doppler) and
plays a chime once at start.
