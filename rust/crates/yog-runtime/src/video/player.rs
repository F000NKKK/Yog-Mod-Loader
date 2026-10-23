//! One player: a video's clock, its decoder and the texture its frame is
//! in, and the sound played with it — started again from where the clock
//! is whenever it plays, is sought or loops round (the hosts start a
//! streamed sound partway).

use std::time::Instant;
use yog_sync::Arc;

use jni::JNIEnv;
use yog_event::{client, video};
use yog_media::video::Decoder;
use yog_registry::{Clock, VideoDef};

/// The pitch a video's sound plays at: as it was made.
const PITCH: f32 = 1.0;

pub(super) struct Player {
    def: Arc<VideoDef>,
    clock: Clock,
    /// When the clock was last moved on.
    last: Instant,
    /// The frame in the texture (none yet).
    shown: Option<u32>,
    decoder: Decoder,
    rgba: Vec<u8>,
    texture: String,
    /// The sound playing with it.
    sound: Option<i32>,
}

impl Player {
    pub(super) fn new(def: Arc<VideoDef>, texture: String) -> Option<Player> {
        let decoder = Decoder::new()
            .map_err(|e| yog_logging::warn!("video {}: {e}", def.id))
            .ok()?;
        Some(Player {
            def,
            clock: Clock::default(),
            last: Instant::now(),
            shown: None,
            decoder,
            rgba: Vec::new(),
            texture,
            sound: None,
        })
    }

    /// Plays or pauses it.
    pub(super) fn play(&mut self, playing: bool) {
        self.tick();
        if playing && !self.clock.playing && self.clock.at >= self.def.seconds() {
            self.clock.at = 0.0;
        }
        self.clock.playing = playing;
        self.restart_sound();
    }

    pub(super) fn seek(&mut self, seconds: f64) {
        self.tick();
        self.clock.seek(&self.def, seconds);
        self.restart_sound();
    }

    /// Its picture now: the clock moved on, the frame it's on decoded into its texture.
    pub(super) fn frame(&mut self, env: &mut JNIEnv) -> Option<video::Picture> {
        let target = self.tick();
        self.show(target)?;
        let (width, height) = (self.def.width, self.def.height);
        if !crate::gfx::textures::update(env, &self.texture, width, height, &self.rgba) {
            return None;
        }
        Some(video::Picture {
            texture: self.texture.clone(),
            width,
            height,
            seconds: self.clock.at,
            length: self.def.seconds(),
            playing: self.clock.playing,
        })
    }

    /// Its picture and sound let go.
    pub(super) fn close(&mut self, env: &mut JNIEnv) {
        self.stop_sound();
        crate::gfx::textures::release(env, &self.texture);
    }

    /// The clock moved on to now: the frame it's on. A loop round starts
    /// its sound again; its end stops it.
    fn tick(&mut self) -> u32 {
        let now = Instant::now();
        let dt = now.duration_since(self.last).as_secs_f64();
        self.last = now;
        let before = self.clock.at;
        let frame = self.clock.advance(&self.def, dt);
        if self.clock.playing && self.clock.at < before {
            self.restart_sound();
        } else if !self.clock.playing {
            self.stop_sound();
        }
        frame
    }

    /// Frame `target` decoded into `rgba` (from the keyframe before it when need be).
    fn show(&mut self, target: u32) -> Option<()> {
        let plan = self.def.plan(self.shown, target);
        if plan.reset {
            self.decoder.reset();
        }
        let size = (self.def.width as usize, self.def.height as usize);
        let packets = plan.frames.clone().filter_map(|f| self.def.packet(f));
        if let Err(e) = self.decoder.decode_last(packets, size, &mut self.rgba) {
            yog_logging::warn!("video {} frame {target}: {e}", self.def.id);
            self.shown = None;
            return None;
        }
        self.shown = plan.frames.clone().next_back().or(self.shown);
        Some(())
    }

    /// Its sound stopped, and started again from where the clock is when it plays.
    fn restart_sound(&mut self) {
        self.stop_sound();
        let Some(sound) = self.def.sound.clone().filter(|_| self.clock.playing) else {
            return;
        };
        self.sound = crate::bus::call(&client::PlaySound {
            sound,
            volume: self.def.volume,
            pitch: PITCH,
            looping: false,
            from: self.clock.at as f32,
        })
        .flatten();
    }

    fn stop_sound(&mut self) {
        if let Some(handle) = self.sound.take() {
            crate::bus::call(&client::StopSound { handle });
        }
    }
}
