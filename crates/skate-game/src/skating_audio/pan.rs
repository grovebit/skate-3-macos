//! Collision-only host mixer. One 48 kHz clock drains commands and renders
//! every voice on the same 256-frame boundary before device resampling.
use bevy::{
    audio::{Decodable, Source},
    prelude::*,
};
use rodio::source::SamplesConverter;
use skate_core::audio::pan;
use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

// Dac0 authored rate, converted by 82B1DC68 and copied to engine +D8.
const OUTPUT_RATE: u32 = 48_000;
const FRAMES: usize = pan::block::BLOCK_SAMPLES;
/// Host linear resampler; fractional position survives block-boundary retuning.
/// Original Rsp0 interpolation/smoothing is still untraced.
struct MonoDecoder {
    source: SamplesConverter<<AudioSource as Decodable>::Decoder, f32>,
    left: Option<f32>,
    right: Option<f32>,
    fraction: f64,
    rate: f64,
    increment: f64,
}
impl MonoDecoder {
    fn new(clip: &AudioSource, speed: f32) -> Self {
        let mut source = clip.decoder().convert_samples::<f32>();
        debug_assert_eq!(
            source.channels(),
            1,
            "collision bank validates mono exports"
        );
        let rate = f64::from(source.sample_rate()) / f64::from(OUTPUT_RATE);
        let left = source.next();
        let right = source.next();
        Self {
            source,
            left,
            right,
            fraction: 0.0,
            rate,
            increment: rate * f64::from(speed),
        }
    }
    fn retune(&mut self, speed: f32) {
        self.increment = self.rate * f64::from(speed);
    }
    fn next(&mut self) -> Option<f32> {
        // A zero pitch (including an absent original controller) retains the
        // voice and cursor in host silence; it must not complete the group.
        if self.increment <= 0.0 {
            return self.left.map(|_| 0.0);
        }
        while self.fraction >= 1.0 {
            self.left = self.right;
            self.right = self.source.next();
            self.fraction -= 1.0;
        }
        let left = self.left?;
        let right = self.right.unwrap_or(left);
        let sample = left + (right - left) * self.fraction as f32;
        self.fraction += self.increment;
        Some(sample)
    }
}
type Queue = Arc<Shared>;
#[derive(Default)]
struct Shared {
    commands: Mutex<Vec<Command>>,
    running: AtomicBool,
    closed: AtomicBool,
}
fn finish_pending(command: &Command) {
    if let Command::Start { control, .. } = command {
        control.finished.store(true, Ordering::Release);
    }
}

#[derive(Component, Clone)]
pub(super) struct CollisionControl {
    id: u64,
    finished: Arc<AtomicBool>,
}
impl CollisionControl {
    pub fn finished(&self) -> bool {
        self.finished.load(Ordering::Acquire)
    }
}

enum Command {
    Start {
        control: CollisionControl,
        source: MonoDecoder,
        degrees: f32,
        volume: f32,
    },
    Update {
        id: u64,
        speed: f32,
        degrees: f32,
        volume: f32,
    },
    Stop(u64),
}

/// Bevy owns proxy entities; the persistent audio source owns PCM voices.
/// Commands produced in a playback pass are published together by `flush`.
#[derive(Resource, Default)]
pub(super) struct CollisionOutput {
    pub entity: Option<Entity>,
    queue: Queue,
    pending: Vec<Command>,
    next_id: u64,
}
impl CollisionOutput {
    pub fn needs_restart(&self) -> bool {
        self.queue.closed.load(Ordering::Acquire)
    }
    pub fn reset_source(&mut self) {
        self.queue = Arc::default();
    }
    pub fn asset(&self) -> CollisionSource {
        CollisionSource {
            queue: self.queue.clone(),
        }
    }
    pub fn start(
        &mut self,
        clip: &AudioSource,
        speed: f32,
        words: &[u32],
        volume: f32,
    ) -> CollisionControl {
        let control = CollisionControl {
            id: self.next_id,
            finished: Arc::new(AtomicBool::new(false)),
        };
        self.next_id = self
            .next_id
            .checked_add(1)
            .expect("collision voice id exhausted");
        self.pending.push(Command::Start {
            control: control.clone(),
            source: MonoDecoder::new(clip, speed),
            degrees: degrees(words),
            volume,
        });
        control
    }
    pub fn update(&mut self, control: &CollisionControl, speed: f32, words: &[u32], volume: f32) {
        self.pending.push(Command::Update {
            id: control.id,
            speed,
            degrees: degrees(words),
            volume,
        });
    }
    pub fn stop(&mut self, control: &CollisionControl) {
        self.pending.push(Command::Stop(control.id));
    }
    pub fn flush(&mut self) {
        if self.pending.is_empty() {
            return;
        }
        let mut queue = self
            .queue
            .commands
            .lock()
            .expect("collision commands poisoned");
        if self.queue.closed.load(Ordering::Acquire) {
            for command in self.pending.drain(..) {
                finish_pending(&command);
            }
            return;
        }
        if self.queue.running.load(Ordering::Acquire) {
            queue.append(&mut self.pending);
            return;
        }
        // Bevy may have no audio device, so decoder creation is not guaranteed.
        // Keep only the live starts and latest controls until output is ready.
        for command in self.pending.drain(..) {
            match command {
                Command::Stop(id) => queue.retain(|queued| {
                    let remove = match queued {
                        Command::Start { control, .. } => control.id == id,
                        Command::Update { id: queued_id, .. } => *queued_id == id,
                        Command::Stop(_) => false,
                    };
                    if remove { finish_pending(queued); }
                    !remove
                }),
                Command::Update { id, speed, degrees, volume } => {
                    if let Some(Command::Update { speed: old_speed, degrees: old_degrees, volume: old_volume, .. }) = queue.iter_mut().find(|queued| matches!(queued, Command::Update { id: queued_id, .. } if *queued_id == id)) {
                        *old_speed = speed;
                        *old_degrees = degrees;
                        *old_volume = volume;
                    } else if queue.iter().any(|queued| matches!(queued, Command::Start { control, .. } if control.id == id)) {
                        queue.push(Command::Update { id, speed, degrees, volume });
                    }
                }
                start => queue.push(start),
            }
        }
    }
}
fn degrees(words: &[u32]) -> f32 {
    pan::collision_degrees(Some(words)).expect("complete controller")
}

/// Constructed once for the persistent collision output entity.
#[derive(Asset, TypePath, Clone)]
pub(super) struct CollisionSource {
    queue: Queue,
}
impl Decodable for CollisionSource {
    type DecoderItem = f32;
    type Decoder = CollisionDecoder;
    fn decoder(&self) -> Self::Decoder {
        let _guard = self
            .queue
            .commands
            .lock()
            .expect("collision commands poisoned");
        assert!(
            !self.queue.closed.load(Ordering::Acquire),
            "closed collision source"
        );
        assert!(
            !self.queue.running.swap(true, Ordering::AcqRel),
            "collision source already playing"
        );
        CollisionDecoder {
            queue: self.queue.clone(),
            voices: Vec::with_capacity(super::collisions::GROUPS * 2),
            commands: Vec::with_capacity(super::collisions::GROUPS * 6),
            pcm: [0.0; FRAMES * 2],
            cursor: FRAMES * 2,
        }
    }
}

struct MixerVoice {
    control: CollisionControl,
    source: MonoDecoder,
    degrees: f32,
    previous_degrees: f32,
    initialized: bool,
    matrix: [f32; 2],
    volume: f32,
}
pub(super) struct CollisionDecoder {
    queue: Queue,
    voices: Vec<MixerVoice>,
    commands: Vec<Command>,
    pcm: [f32; FRAMES * 2],
    cursor: usize,
}
impl CollisionDecoder {
    fn render(&mut self) {
        // Release the command lock before reading PCM or computing ramps.
        {
            let mut queued = self
                .queue
                .commands
                .lock()
                .expect("collision commands poisoned");
            std::mem::swap(&mut *queued, &mut self.commands);
        }
        {
            for command in self.commands.drain(..) {
                match command {
                    Command::Start {
                        control,
                        source,
                        degrees,
                        volume,
                    } => {
                        // A newly activated graph initializes Pn21 directly.
                        self.voices.push(MixerVoice {
                            control,
                            source,
                            degrees,
                            previous_degrees: degrees,
                            matrix: [0.0; 2],
                            initialized: false,
                            volume,
                        });
                    }
                    Command::Update {
                        id,
                        speed,
                        degrees,
                        volume,
                    } => {
                        if let Some(voice) = self.voices.iter_mut().find(|v| v.control.id == id) {
                            voice.source.retune(speed);
                            voice.degrees = degrees;
                            voice.volume = volume;
                        }
                    }
                    Command::Stop(id) => {
                        if let Some(index) = self.voices.iter().position(|v| v.control.id == id) {
                            self.voices
                                .remove(index)
                                .control
                                .finished
                                .store(true, Ordering::Release);
                        }
                    }
                }
            }
        }
        self.pcm.fill(0.0);
        for voice in &mut self.voices {
            let ramp = if !voice.initialized {
                voice.matrix = pan::mono_stereo(voice.degrees);
                voice.previous_degrees = voice.degrees;
                voice.initialized = true;
                None
            } else if voice.degrees != voice.previous_degrees {
                let next = pan::mono_stereo(voice.degrees);
                let ramp = std::array::from_fn::<_, 2, _>(|channel| {
                    pan::block::channel_gains(voice.matrix[channel], next[channel])
                });
                // The next block uses the stored target, not the rounded tail.
                voice.matrix = next;
                voice.previous_degrees = voice.degrees;
                Some(ramp)
            } else {
                None
            };
            for frame in 0..FRAMES {
                let Some(mono) = voice.source.next() else {
                    voice.control.finished.store(true, Ordering::Release);
                    break;
                };
                for channel in 0..2 {
                    let gain = ramp
                        .as_ref()
                        .map_or(voice.matrix[channel], |r| r[channel][frame]);
                    self.pcm[frame * 2 + channel] += (mono * gain) * voice.volume;
                }
            }
        }
        self.voices.retain(|voice| !voice.control.finished());
        self.cursor = 0;
    }
}
impl Iterator for CollisionDecoder {
    type Item = f32;
    fn next(&mut self) -> Option<f32> {
        if self.cursor == self.pcm.len() {
            self.render();
        }
        let sample = self.pcm[self.cursor];
        self.cursor += 1;
        Some(sample)
    }
}
impl Source for CollisionDecoder {
    fn current_frame_len(&self) -> Option<usize> {
        None
    }
    fn channels(&self) -> u16 {
        2
    }
    fn sample_rate(&self) -> u32 {
        OUTPUT_RATE
    }
    fn total_duration(&self) -> Option<Duration> {
        None
    }
}
impl Drop for CollisionDecoder {
    fn drop(&mut self) {
        let mut queued = self
            .queue
            .commands
            .lock()
            .expect("collision commands poisoned");
        self.queue.closed.store(true, Ordering::Release);
        self.queue.running.store(false, Ordering::Release);
        for command in queued.drain(..) {
            finish_pending(&command);
        }
        for voice in &self.voices {
            voice.control.finished.store(true, Ordering::Release);
        }
    }
}

#[cfg(test)]
mod tests;
