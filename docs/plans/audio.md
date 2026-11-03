# Audio (AUDIO-1)

## Goal

Objects and points in a scene make sound that is heard from the active camera, with
distance, direction and Doppler. Clips play once or loop. An IDE Audio editor imports,
trims and previews clips, and generates sound effects, music and speech with fal.

## Current state

- **AUDIO-1a delivered (2026-11-02):** `src/audio.rs` and `examples/audio_bench.rs`.
  Measured on an M3 Pro, release build, 64 moving sources, 32 real voices, 48 kHz:
  update p50 1.9 µs / p99 7.5 µs; one 512-frame mixer block p50 51 µs / p99 109 µs
  (block period 10.7 ms). Bus and master gains are folded into each voice's gains on
  the engine thread, so the mixer has no bus stage until effects need one.
- **AUDIO-1b delivered (2026-11-02):** `src/audio/asset.rs` (clip assets, components,
  loader), `src/audio/output.rs` (cpal), host wiring in `src/project_host/visual.rs`,
  `audio` feature enabled in the template and `first_project`, and
  `assets/scenes/audio-demo.scene.toml` (a humming cube orbiting past the camera plus a
  one-shot chime; run with `--run-scene assets/scenes/audio-demo.scene.toml`).
  Differences from the design below, kept deliberately small:
  - Clips decode on a background loader thread when a sound first needs them, not at
    scene validation (the IDE validates on every edit). Scene validation only parses the
    `.audio.toml` and checks the source file exists and fits the budget. A source starts
    a few milliseconds after load. Games can decode synchronously with
    `AudioWorld::load_clip`.
  - Clips keep their own sample rate; the mixer's per-voice linear resampler converts,
    so there's no separate resampling pass.
  - The bus comes from the clip's `category`; the source component has no bus field.
  - Removing the `audio_source` component without deleting the entity leaves its sound
    playing until it ends (deleting the entity stops it).
- **AUDIO-1c delivered (2026-11-02):** the `/audio` editor (library, import, waveform with
  trim/loop handles, gain and normalise, fades, preview, engine-validated saves through
  `--inspect-audio`, add a sound point to a scene, delete when unused), range rings for a
  selected source in the scene viewer, clips in the project index, and AI chat/MCP guidance.
  Clip assets gained `fade_in` / `fade_out`, and a trim past the end of the audio is refused.
- **AUDIO-1d delivered (2026-11-02):** fal generation in the editor's Generate tab. Defaults,
  chosen from the live catalogue on 2026-11-02: `fal-ai/elevenlabs/sound-effects/v2`
  (seamless-loop option), `fal-ai/elevenlabs/music` (instrumental option) and
  `fal-ai/elevenlabs/tts/eleven-v3` (preset voice names). Other `text-to-audio` /
  `text-to-speech` models are adapted from their schema (text or prompt, a duration field in
  seconds or milliseconds, seed, voice, URL output); models needing other required inputs
  are refused before any paid call. Verified with one live job per mode, each decoded by the
  engine. Known gaps: MiniMax voices sit in a nested `voice_setting` (the default voice is
  used), and ElevenLabs v4's schema needs a dedicated adapter.
- **AUDIO-1e delivered (2026-11-02):**
  - Streaming (`src/audio/stream.rs`): a clip with `stream = true` (default for the music
    category) plays from a decoder thread that sends stereo chunks ~0.7 s ahead; used
    chunks are recycled, so the audio thread neither allocates nor frees. Trim, gain, fades
    and loop points behave as for decoded clips (each loop pass reopens the file). An
    underrun plays silence and holds position. `--inspect-audio` validates streamed clips
    through the same path, so they have no 10-minute decode limit.
  - Play on signal: `emergent.audio_source` `play_on` names a brain signal; the clip plays
    when this object starts sending it, or another brain starts sending it at this object.
  - Mixer (`src/audio/mixer.rs`): a Freeverb-style send reverb (listener `reverb_room`,
    `reverb_damping`; source `reverb` send), music ducking while voice-bus sounds play
    (listener `ducking`, default -9 dB, 0.15 s down / 0.6 s up), and an output limiter at
    0.98 replacing the hard clip. Measured render cost: 51 → 73 µs per 512-frame block for
    64 sources (80 µs with reverb), against a 10.7 ms block.
  - MCP `generate_audio` (needs `confirm_paid: true`; waits up to ~150 s), and fal adapters
    for schema paths under a different owner prefix (ElevenLabs v4) and voices nested in
    `voice_setting` (MiniMax), with voice suggestions from the schema. Both verified live.
- Before this slice there was no audio code. GOALS lists audio as a capability family (devices, clips, buses,
  spatial sources/listeners, attenuation, Doppler, voice management) and a Stage gate:
  "establish the audio device, clip/stream, mixer-bus, and spatial source/listener
  boundaries without coupling audio identity to entities".
- Reusable pieces: the component registry (`src/component.rs`) for authored behaviour
  hooks, `ActiveCamera` (`src/motion.rs`), `marker.point` for points in space, the
  fixed-step engine loop, durable asset IDs, and the IDE's fal job pattern
  (`texture-provider.ts`, `fal-models.ts` catalogue with `FalModelKind`).

## Design

### Ownership and identity

- New `src/audio.rs`, behind an `audio` cargo feature like `graphics`. The mixing core
  has no device dependency, so it runs headless and in tests.
- `AudioWorld` (engine thread) owns `SlotMap<ClipId, Clip>` (decoded PCM, shared with the
  mixer as immutable `Arc<[f32]>`) and `SlotMap<SoundId, Sound>` (one playing instance).
  Typed keys; durable clip asset IDs map to fresh `ClipId`s at scene load, never saved.
- Entities refer to audio through components; audio identity is not the entity. A
  stale `SoundId` returns `None` / a typed error, never a different sound. Deleting an
  entity stops its sounds at the next sync.
- The mixer (audio thread) holds a fixed-size voice array. The engine sends commands
  (play, stop, set params) through a preallocated lock-free SPSC ring; the callback never
  locks or allocates. Finished-voice notices come back on a second ring.

### Components (registry, so the IDE, MCP and `set_component` get them free)

- `emergent.audio_listener`: on a camera. `gain`. If no listener is authored, the
  `ActiveCamera` is the listener. At most one is active; a second one is a validation
  warning, and the active camera's wins.
- `emergent.audio_source`: on any instance (a `marker.point` for a fixed point).
  Fields: `clip` (asset ref), `mode` (`one_shot` | `loop`), `autoplay`, `volume`,
  `pitch`, `spatial` (0 = 2D/UI/music, 1 = full 3D), `min_distance`, `max_distance`,
  `rolloff` (`inverse` | `linear`), `doppler` (scale, 0 disables), `bus`
  (`sfx` | `music` | `voice` | `ui`).
- Game/Rust API: `audio.play(clip, PlayAt::Entity(id) | PlayAt::Point(pos) | PlayAt::Flat)
  -> SoundId`, `stop`, `set_volume`. Brain follow-up: an **On signal → play clip** hook.

### Per tick (engine thread, bounded)

1. Read listener global transform; for each live source entity, read its global
   position.
2. Velocity = position delta / fixed dt (works for physics, tweens, brains and
   parenting alike). A jump above a threshold (teleport/respawn) zeroes velocity for
   that tick so it doesn't produce a pitch spike.
3. Compute per voice: distance gain (min/max, rolloff), equal-power stereo pan from the
   direction in listener space, and Doppler pitch
   `f = (c − v_listener·d) / (c − v_source·d)` along the source→listener unit vector `d`,
   `c = 343 m/s`, scaled by `doppler`, clamped to 0.5–2.0.
4. Push one compact param update per changed voice into the ring. Work lists are
   reused `Vec`s; no allocation in steady state.

### Mixer (audio thread)

- Per voice: linear-interpolated resampling at `clip_rate / device_rate × pitch × doppler`;
  gain and pan ramp across the block (no zipper noise or clicks). Loops wrap at the
  clip's loop points; one-shots release the voice at the end.
- Buses `master → sfx / music / voice / ui` with gains. Effects (reverb, filters, ducking)
  are out of the first slice.
- Voice budget: 32 real voices (initial). Beyond it the quietest or furthest are
  virtualised: they keep advancing their play position but aren't mixed.
- Output through `cpal` (focused device crate). Decoding through `symphonia`
  (WAV, MP3, OGG/Vorbis, FLAC; only those features enabled), done at scene load off the
  frame path and resampled to the device rate once. Clips over ~20 s (music) stream from
  a decode thread into a ring instead of decoding fully. A null backend renders to a
  buffer for headless runs and tests.

### Asset format

`assets/audio/<id>/` holds the source file (`clip.wav` / `.mp3` / `.ogg`) and
`<name>.audio.toml` (as delivered):

```toml
schema_version = 1
id = "…"            # durable ID
name = "Engine hum"
kind = "audio"
source = "clip.wav"  # file name in the same folder
category = "sfx"    # sfx | music | voice | ui; picks the bus
gain_db = 0.0
trim = [0.0, 1.42]  # seconds, non-destructive
loop = [0.10, 1.40] # optional loop points
[generation]        # present when fal made it
model = "…"; prompt = "…"; seed = 1
```

Budgets per clip (initial): 50 MiB file, 10 min, 48 kHz, stereo; larger or malformed files
are rejected with a typed error.

### IDE Audio editor (`/audio`)

- Library: list, search, import (drag a file), rename, delete, kind/bus.
- Editor: waveform (Web Audio decode + canvas), play/stop/loop preview, drag trim and
  loop handles, gain, fade in/out, normalise suggestion. Edits write the `.audio.toml`
  only; the source file is untouched.
- **Generate** tab with three modes, using the existing fal job pattern (submit, poll,
  candidates, keep one): **Sound effect** (prompt, duration), **Music** (prompt,
  duration, instrumental/lyrics) and **Speech** (text, voice, speed). `FalModelKind`
  gains `sound`, `music` and `speech`, backed by fal's text-to-audio and
  text-to-speech catalogue categories so users can pick other models. Default endpoints
  get chosen and verified against the live catalogue in that slice (candidates: an
  ElevenLabs or Stable Audio sound-effects model, a music model, and an ElevenLabs,
  MiniMax or Kokoro TTS model). The prompt, model and seed are saved in `[generation]`.
- Scene authoring: the components inspector gets clip pickers for audio sources; the
  scene viewer draws min/max distance spheres for the selected source and an icon for
  the listener. Place from the library adds a `marker.point` with an `audio_source`.

### MCP

Clip assets appear in `find`/`describe` (kind `audio`). Sources and listeners are set with
`set_component`. A later `generate_audio` tool reuses the IDE generator.

## Hot paths and scale

- Engine tick: O(active sources); target < 0.1 ms for 64 sources (to be measured).
- Audio callback: O(real voices × block); must never block, lock or allocate. Measure
  callback time against the block period at 32 voices with Doppler and resampling.
- Memory: decoded PCM is the main cost (≈ 23 MiB per stereo minute at 48 kHz f32), which
  is why long music streams.

## Validation

- Engine (headless, null backend): one-shot ends and frees its voice; loop wraps at loop
  points without a click (sample continuity check); distance gain and pan match the
  formulas; Doppler pitch for approaching/receding sources and a moving listener; teleport
  gives no spike; stale `SoundId` and deleted entity behaviour; voice virtualisation
  beyond budget; malformed/oversized clips rejected; clip ID mapping isn't persisted.
- Bench: tick with 64 sources; mixer block with 32 voices (optimised build, record
  hardware).
- Device: `first_project` demo — a looping engine hum on a moving object passing the
  camera, audibly Doppler-shifted; a one-shot on a brain signal. Run on port 3001.
- IDE: import, trim/loop save round-trip, each fal mode producing a saved clip, the
  template still building (`cargo test`).

## Slices

1. **AUDIO-1a: core, headless (delivered).** `audio.rs`, clip and sound stores, mixer, spatial gain,
   pan, Doppler, buses, voice budget, null backend, tests and bench.
2. **AUDIO-1b: device, assets and components (delivered).** `cpal` output and `symphonia` decoding,
   `.audio.toml`, `audio_source` / `audio_listener` registry components, scene load,
   project host wiring (lives in `src/project_host/`, so every project gets it), template
   default, `first_project` demo.
3. **AUDIO-1c: IDE Audio editor (delivered).** Library, import, waveform, preview, trim/loop/gain,
   inspector clip picker, viewer gizmos, place from library.
4. **AUDIO-1d: fal generation (delivered).** Sound effect, music and speech modes, catalogue kinds,
   candidates, saved provenance.
5. **AUDIO-1e: follow-ups (delivered).** Music streaming, brain "play on signal", MCP
   `generate_audio`, ducking/reverb, occlusion.

## Decisions

- Own mixer on `cpal`, not `kira` (2026-11-02).
- The `graphics` feature implies `audio` (2026-11-02): the project host is the shared IDE
  contract, so projects created before the template enabled `audio` still decode clips.
- Audio is output only: parameters come from fixed-tick transforms, so they are
  repeatable for a given simulation, but mixed samples are not part of replay.

## Still open

- Occlusion (sound through walls) and reverb zones per area.
- Music transitions and crossfades between tracks.
- ElevenLabs account voices in the voice picker (presets only today).
