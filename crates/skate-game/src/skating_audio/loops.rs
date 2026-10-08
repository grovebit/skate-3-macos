//! Host loop envelopes, independent of render frame rate.
#[derive(Clone, Copy, Debug)]
pub(super) struct Envelope {
    pub fade: f32,
    pub gain: f32,
    pub pitch: f32,
}
impl Default for Envelope {
    fn default() -> Self {
        Self {
            fade: 0.,
            gain: 0.,
            pitch: 1.,
        }
    }
}
impl Envelope {
    pub fn update(&mut self, selected: bool, speed: f32, dt: f32) {
        let dt = if dt.is_finite() {
            dt.clamp(0., 0.25)
        } else {
            0.
        };
        let amount = dt / 0.08;
        self.fade = if selected {
            (self.fade + amount).min(1.)
        } else {
            (self.fade - amount).max(0.)
        };
        // Retain outgoing speed/gain through the fade; an airborne or bail
        // state can replace the simulation's speed before the sound tails off.
        if selected {
            let speed = if speed.is_finite() {
                speed.clamp(0., 30.)
            } else {
                0.
            };
            let blend = 1. - (-dt / 0.06).exp();
            self.gain += ((speed / 8.).clamp(0., 1.) - self.gain) * blend;
            self.pitch += ((0.85 + speed * 0.025).clamp(0.85, 1.25) - self.pitch) * blend;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn smoothing_is_consistent_across_render_rates() {
        let run = |hz| {
            let mut envelope = Envelope::default();
            for _ in 0..hz {
                envelope.update(true, 12., 1. / hz as f32);
            }
            envelope
        };
        let a = run(30);
        let b = run(200);
        assert!((a.gain - b.gain).abs() < 0.0001);
        assert!((a.pitch - b.pitch).abs() < 0.0001);
        assert_eq!(a.fade, 1.);
        assert_eq!(b.fade, 1.);
    }
    #[test]
    fn outgoing_loop_keeps_its_speed_and_crossfades_to_incoming_loop() {
        let mut outgoing = Envelope {
            fade: 1.,
            gain: 0.8,
            pitch: 1.2,
        };
        let mut incoming = Envelope::default();
        for _ in 0..4 {
            outgoing.update(false, 0., 0.01);
            incoming.update(true, 8., 0.01);
            assert!((outgoing.fade + incoming.fade - 1.).abs() < 0.0001);
        }
        assert_eq!(outgoing.gain, 0.8);
        assert_eq!(outgoing.pitch, 1.2);
        assert!(outgoing.fade > 0. && outgoing.fade < 1.);
        outgoing.update(false, 0., 0.08);
        assert_eq!(outgoing.fade, 0.);
    }
    #[test]
    fn rapid_return_reverses_fade_without_restarting_and_invalid_time_is_ignored() {
        let mut envelope = Envelope {
            fade: 1.,
            gain: 1.,
            pitch: 1.,
        };
        envelope.update(false, 0., 0.02);
        let previous = envelope.fade;
        envelope.update(true, 8., 0.01);
        assert!(envelope.fade > previous && envelope.fade < 1.);
        let previous = envelope;
        envelope.update(false, f32::NAN, f32::NAN);
        assert_eq!(envelope.fade, previous.fade);
        envelope.update(false, 0., 10.);
        assert_eq!(envelope.fade, 0.);
    }
}
