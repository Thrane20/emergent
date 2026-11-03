//! Audio clip assets, scene audio components and end-to-end playback through the engine.
use emergent::{AudioClipAsset, AudioEmitter, AudioListener, Bus, Project, SceneDefinition, World};
use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

static NEXT: AtomicU64 = AtomicU64::new(0);
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "emergent-audio-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(root.join("assets/audio/hum")).unwrap();
        fs::create_dir_all(root.join("src/generated")).unwrap();
        fs::write(root.join("emergent.toml"), "schema_version = 1\nid = 'fixture'\nname = 'Fixture'\nentry_scene = 'assets/main.scene.toml'\ngenerated_code = 'src/generated'\nasset_roots = ['assets']\n[[outputs]]\nid = 'host'\nos = 'macos'\narchitecture = 'aarch64'\nrust_target = 'aarch64-apple-darwin'\nprofile = 'development'\n").unwrap();
        fs::write(
            root.join("assets/main.scene.toml"),
            "schema_version = 1\nid = 'main'\n",
        )
        .unwrap();
        fs::write(root.join("assets/cube.entity.toml"), "schema_version=1\nid='cube'\nkind='primitive'\n[geometry]\nprimitive='cube'\nhalf_extent=0.5\ncolor=[1,1,1,1]\n").unwrap();
        // One second of a 16-bit mono 440 Hz tone at 8 kHz.
        let samples: Vec<i16> = (0..8000)
            .map(|i| ((i as f32 / 8000.0 * 440.0 * std::f32::consts::TAU).sin() * 16000.0) as i16)
            .collect();
        fs::write(
            root.join("assets/audio/hum/hum.wav"),
            wav(&samples, 1, 8000),
        )
        .unwrap();
        Self(root)
    }
    fn project(&self) -> Project {
        Project::load(&self.0).unwrap()
    }
    fn clip(&self, body: &str) -> Result<AudioClipAsset, String> {
        fs::write(
            self.0.join("assets/audio/hum/hum.audio.toml"),
            format!("schema_version = 1\nid = 'hum'\nname = 'Hum'\nkind = 'audio'\n{body}"),
        )
        .unwrap();
        AudioClipAsset::load(&self.project(), "assets/audio/hum/hum.audio.toml")
    }
    fn scene(&self, source: &str) -> Result<SceneDefinition, emergent::SceneError> {
        fs::write(
            self.0.join("assets/main.scene.toml"),
            format!("schema_version = 1\nid = 'main'\n{source}"),
        )
        .unwrap();
        SceneDefinition::load(&self.project(), "assets/main.scene.toml")
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn wav(samples: &[i16], channels: u16, rate: u32) -> Vec<u8> {
    let data = (samples.len() * 2) as u32;
    let mut out = Vec::with_capacity(44 + data as usize);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + data).to_le_bytes());
    out.extend_from_slice(b"WAVEfmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&channels.to_le_bytes());
    out.extend_from_slice(&rate.to_le_bytes());
    out.extend_from_slice(&(rate * channels as u32 * 2).to_le_bytes());
    out.extend_from_slice(&(channels * 2).to_le_bytes());
    out.extend_from_slice(&16u16.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&data.to_le_bytes());
    for sample in samples {
        out.extend_from_slice(&sample.to_le_bytes());
    }
    out
}

#[test]
fn clip_assets_are_validated() {
    let f = Fixture::new();
    let clip = f.clip("source = 'hum.wav'\ncategory = 'music'\n").unwrap();
    assert_eq!((clip.bus, clip.id.as_str()), (Bus::Music, "hum"));
    for (body, message) in [
        ("source = '../hum.wav'\n", "same folder"),
        ("source = 'hum.txt'\n", "same folder"),
        ("source = 'gone.wav'\n", "gone.wav"),
        ("source = 'hum.wav'\ncategory = 'noise'\n", "category"),
        ("source = 'hum.wav'\ngain_db = 90\n", "gain_db"),
        ("source = 'hum.wav'\ntrim = [0.5, 0.2]\n", "trim"),
        (
            "source = 'hum.wav'\ntrim = [0.0, 0.5]\nloop = [0.0, 0.8]\n",
            "inside the trimmed",
        ),
        ("source = 'hum.wav'\nvolume = 1\n", "unknown field"),
    ] {
        let error = f.clip(body).unwrap_err();
        assert!(error.contains(message), "{message}: {error}");
    }
}

#[test]
fn scene_audio_components_attach_and_validate() {
    let f = Fixture::new();
    f.clip("source = 'hum.wav'\n").unwrap();
    let scene = f.scene("[[entities]]\nid='car'\nasset='assets/cube.entity.toml'\n[[entities.components]]\ntype='emergent.audio_source'\nclip='assets/audio/hum/hum.audio.toml'\nmode='one_shot'\nmax_distance=80\n[[entities]]\nid='ears'\nasset='assets/cube.entity.toml'\n[[entities.components]]\ntype='emergent.audio_listener'\ngain=0.5\n").unwrap();
    let mut world = World::new();
    let loaded = scene.instantiate(&mut world).unwrap();
    let emitter = world
        .component::<AudioEmitter>(loaded.entities["car"])
        .unwrap()
        .unwrap();
    assert!(!emitter.settings.looping);
    assert_eq!(emitter.settings.max_distance, 80.0);
    assert_eq!(emitter.settings.min_distance, 1.0, "schema default");
    let listener = world
        .component::<AudioListener>(loaded.entities["ears"])
        .unwrap()
        .unwrap();
    assert_eq!(listener.gain, 0.5);
    for (component, message) in [
        (
            "clip='assets/audio/hum/hum.audio.toml'\nmin_distance=10\nmax_distance=5\n",
            "min_distance",
        ),
        (
            "clip='assets/audio/missing.audio.toml'\n",
            "missing.audio.toml",
        ),
        ("clip='assets/audio/hum/hum.wav'\n", ".audio.toml"),
    ] {
        let error = f
            .scene(&format!("[[entities]]\nid='a'\nasset='assets/cube.entity.toml'\n[[entities.components]]\ntype='emergent.audio_source'\n{component}"))
            .err()
            .unwrap()
            .to_string();
        assert!(error.contains(message), "{message}: {error}");
    }
}

#[cfg(feature = "audio")]
mod playback {
    use super::*;
    use emergent::{
        AudioConfig, AudioWorld, Engine, EngineConfig, EngineContext, Game, PlayAt, SoundSettings,
    };
    use std::time::Duration;

    struct Idle;
    impl Game for Idle {
        fn fixed_update(&mut self, _: &mut EngineContext<'_>) {}
    }

    #[test]
    fn wav_clips_decode_with_trim_gain_and_loop() {
        let f = Fixture::new();
        let full = f.clip("source = 'hum.wav'\n").unwrap().decode().unwrap();
        assert_eq!(
            (full.channels(), full.sample_rate(), full.frames()),
            (1, 8000, 8000)
        );
        let trimmed = f
            .clip("source = 'hum.wav'\ntrim = [0.25, 0.75]\nloop = [0.1, 0.4]\ngain_db = -6\n")
            .unwrap()
            .decode()
            .unwrap();
        assert_eq!(trimmed.frames(), 4000);
        // 0.53 s at 48 kHz is 25439.998 in f32: a loop ending at the trim end must still fit.
        let tone: Vec<i16> = (0..48_000).map(|i| ((i % 100) as i16 - 50) * 300).collect();
        fs::write(
            f.0.join("assets/audio/hum/hum48.wav"),
            wav(&tone, 1, 48_000),
        )
        .unwrap();
        let edge = f
            .clip("source = 'hum48.wav'\ntrim = [0.0, 0.53]\nloop = [0.0, 0.53]\n")
            .unwrap()
            .decode()
            .unwrap();
        assert_eq!(edge.frames(), 25_440);
    }

    #[test]
    fn fades_ramp_the_ends_and_trims_past_the_end_are_refused() {
        let f = Fixture::new();
        let plain = f.clip("source = 'hum.wav'\n").unwrap().decode().unwrap();
        let faded = f
            .clip("source = 'hum.wav'\nfade_in = 0.5\nfade_out = 0.5\n")
            .unwrap()
            .decode()
            .unwrap();
        let (plain, faded) = (plain.samples(), faded.samples());
        // 8 kHz: frame 1001 is about a quarter of the way through the half-second fade in.
        let ratio = |i: usize| faded[i] / plain[i];
        assert!((ratio(1001) - 0.25).abs() < 0.01, "{}", ratio(1001));
        assert!((ratio(6998) - 0.25).abs() < 0.01, "{}", ratio(6998));
        assert_eq!(faded[0], 0.0);
        assert_eq!(faded[7999], 0.0);
        assert!(f
            .clip("source = 'hum.wav'\ntrim = [0.0, 5.0]\n")
            .unwrap()
            .decode()
            .is_err());
        assert!(f.clip("source = 'hum.wav'\nfade_in = 90\n").is_err());
    }

    /// Renders `blocks` blocks of 400 frames, waiting briefly whenever the stream is starved.
    fn render_stream(mixer: &mut emergent::Mixer, frames: usize) -> Vec<f32> {
        let mut out = vec![0.0; frames * 2];
        // Let the decoder thread fill its queue before the first block.
        std::thread::sleep(Duration::from_millis(50));
        for block in out.chunks_mut(800) {
            mixer.render(block);
            std::thread::sleep(Duration::from_millis(1));
        }
        out
    }

    #[test]
    fn streamed_clips_match_decoded_ones_and_loop_seamlessly() {
        let f = Fixture::new();
        let asset = std::sync::Arc::new(
            f.clip("source = 'hum.wav'\nstream = true\nloop = [0.0, 1.0]\ngain_db = -6\n")
                .unwrap(),
        );
        assert!(asset.streams());
        let decoded = asset.decode().unwrap();
        // The device runs at the clip's rate, so streamed frames map 1:1 onto clip frames.
        let config = AudioConfig {
            device_rate: 8000,
            ..AudioConfig::default()
        };
        let (mut audio, mut mixer) = AudioWorld::new(config).unwrap();
        let world = emergent::World::new();
        let settings = SoundSettings {
            looping: true,
            spatial: 0.0,
            reverb: 0.0,
            ..Default::default()
        };
        let sound = audio.play_stream(&asset, PlayAt::Flat, settings).unwrap();
        audio.update(&world, Duration::from_millis(10));
        let out = render_stream(&mut mixer, 20_000);
        let left: Vec<f32> = out.chunks(2).map(|frame| frame[0]).collect();
        // The stream starts one frame late while it interpolates from silence.
        let offset = (0..4)
            .min_by(|a, b| {
                let error = |o: usize| {
                    (0..1000)
                        .map(|i| (left[i + o] - decoded.samples()[i]).abs())
                        .sum::<f32>()
                };
                error(*a).total_cmp(&error(*b))
            })
            .unwrap();
        for i in 0..7000 {
            assert!(
                (left[i + offset] - decoded.samples()[i]).abs() < 1e-4,
                "frame {i}"
            );
        }
        // Past one second the loop wraps; a 440 Hz tone over 8 kHz moves < 0.6 per frame
        // at -6 dB, so any gap or click would show as a larger step.
        let max_step = left[offset..]
            .windows(2)
            .map(|w| (w[1] - w[0]).abs())
            .fold(0.0, f32::max);
        assert!(max_step < 0.2, "max step {max_step}");
        assert!(audio.is_playing(sound));
    }

    #[test]
    fn a_stream_that_does_not_loop_ends() {
        let f = Fixture::new();
        let asset = std::sync::Arc::new(
            f.clip("source = 'hum.wav'\nstream = true\ntrim = [0.0, 0.25]\n")
                .unwrap(),
        );
        let config = AudioConfig {
            device_rate: 8000,
            ..AudioConfig::default()
        };
        let scan = asset.scan_stream().unwrap();
        assert_eq!(scan.frames, 2000);
        assert_eq!(
            scan.blocks.len(),
            2000usize.div_ceil(emergent::audio::SCAN_BLOCK)
        );
        assert!(scan
            .blocks
            .iter()
            .all(|level| *level > 0.1 && *level <= scan.peak));
        let past_end = std::sync::Arc::new(
            f.clip("source = 'hum.wav'\nstream = true\ntrim = [0.0, 5.0]\n")
                .unwrap(),
        );
        assert!(
            past_end.scan_stream().is_err(),
            "a trim past the end is refused"
        );
        let (mut audio, mut mixer) = AudioWorld::new(config).unwrap();
        let world = emergent::World::new();
        let sound = audio
            .play_stream(&asset, PlayAt::Flat, SoundSettings::default())
            .unwrap();
        audio.update(&world, Duration::from_millis(10));
        let out = render_stream(&mut mixer, 4000);
        let heard = out.iter().filter(|s| s.abs() > 0.01).count() / 2;
        assert!(
            (1500..=2100).contains(&heard),
            "{heard} frames of a 2000-frame clip"
        );
        audio.update(&world, Duration::from_millis(10));
        assert!(!audio.is_playing(sound));
    }

    #[test]
    fn a_scene_source_plays_through_the_engine_and_stops_with_its_entity() {
        let f = Fixture::new();
        f.clip("source = 'hum.wav'\nloop = [0.0, 1.0]\n").unwrap();
        let scene = f.scene("[[entities]]\nid='car'\nasset='assets/cube.entity.toml'\n[entities.transform]\ntranslation=[2,0,0]\n[[entities.components]]\ntype='emergent.audio_source'\nclip='assets/audio/hum/hum.audio.toml'\n").unwrap();
        let mut engine = Engine::new(EngineConfig::default()).unwrap();
        let (audio, mut mixer) = AudioWorld::new(AudioConfig::default()).unwrap();
        engine.set_audio(audio);
        let loaded = scene.instantiate(engine.world_mut()).unwrap();
        let car = loaded.entities["car"];
        // The clip decodes on the loader thread; give it a bounded wait.
        let mut sound = None;
        for _ in 0..500 {
            engine.run_ticks(&mut Idle, 1).unwrap();
            sound = engine
                .world()
                .component::<AudioEmitter>(car)
                .unwrap()
                .unwrap()
                .sound();
            if sound.is_some() {
                break;
            }
            std::thread::sleep(Duration::from_millis(2));
        }
        let sound = sound.expect("the clip loaded and started");
        engine.run_ticks(&mut Idle, 1).unwrap();
        let mut out = vec![0.0; 1024];
        mixer.render(&mut out);
        let (left, right) = out.chunks(2).fold((0.0f32, 0.0f32), |(l, r), s| {
            (l.max(s[0].abs()), r.max(s[1].abs()))
        });
        assert!(
            right > 0.1 && right > left,
            "source on the right: {left} {right}"
        );
        engine.world_mut().remove_subtree(car).unwrap();
        engine.run_ticks(&mut Idle, 1).unwrap();
        assert!(!engine.audio_mut().unwrap().is_playing(sound));
    }
}
