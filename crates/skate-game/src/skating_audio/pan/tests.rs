use super::*;
fn mono_clip(rate: u32, samples: &[i16]) -> AudioSource {
    let size = (samples.len() * 2) as u32;
    let mut wav = Vec::new();
    wav.extend_from_slice(b"RIFF");
    wav.extend_from_slice(&(36 + size).to_le_bytes());
    wav.extend_from_slice(b"WAVEfmt ");
    wav.extend_from_slice(&16u32.to_le_bytes());
    wav.extend_from_slice(&1u16.to_le_bytes());
    wav.extend_from_slice(&1u16.to_le_bytes());
    wav.extend_from_slice(&rate.to_le_bytes());
    wav.extend_from_slice(&(rate * 2).to_le_bytes());
    wav.extend_from_slice(&2u16.to_le_bytes());
    wav.extend_from_slice(&16u16.to_le_bytes());
    wav.extend_from_slice(b"data");
    wav.extend_from_slice(&size.to_le_bytes());
    for sample in samples {
        wav.extend_from_slice(&sample.to_le_bytes());
    }
    AudioSource { bytes: wav.into() }
}

fn pair(decoder: &mut CollisionDecoder) -> [f32; 2] {
    [decoder.next().unwrap(), decoder.next().unwrap()]
}
fn skip(decoder: &mut CollisionDecoder, frames: usize) {
    for _ in 0..frames {
        pair(decoder);
    }
}
fn gains(word: u32) -> [f32; 2] {
    pan::mono_stereo(degrees(&[word]))
}

#[test]
fn initial_block_uses_latest_controls_without_ramping() {
    let mut output = CollisionOutput::default();
    let control = output.start(&mono_clip(48_000, &[16_384; 512]), 1.0, &[0], 1.0);
    output.flush();
    let mut decoder = output.asset().decoder();
    output.update(&control, 1.0, &[49_151], 0.5);
    output.flush();
    for _ in 0..FRAMES {
        assert_eq!(pair(&mut decoder), gains(49_151).map(|g| g * 0.25));
    }
}

#[test]
fn absent_output_coalesces_controls_and_reclaims_stopped_starts() {
    let mut output = CollisionOutput::default();
    let control = output.start(&mono_clip(48_000, &[16_384; 512]), 1.0, &[0], 1.0);
    output.flush();
    for word in 0..10_000 {
        output.update(&control, 1.0, &[word], 1.0);
        output.flush();
    }
    assert_eq!(output.queue.commands.lock().unwrap().len(), 2);
    let mut decoder = output.asset().decoder();
    assert_eq!(pair(&mut decoder), gains(9_999).map(|g| g * 0.5));
    drop(decoder);
    assert!(control.finished());
    assert!(output.needs_restart());
    output.reset_source();
    let stopped = output.start(&mono_clip(48_000, &[16_384; 512]), 1.0, &[0], 1.0);
    output.flush();
    output.stop(&stopped);
    output.flush();
    assert!(stopped.finished());
    assert!(output.queue.commands.lock().unwrap().is_empty());
}

#[test]
fn decoder_drop_finishes_active_and_queued_starts_then_can_restart() {
    let mut output = CollisionOutput::default();
    let active = output.start(&mono_clip(48_000, &[16_384; 512]), 1.0, &[0], 1.0);
    output.flush();
    let mut decoder = output.asset().decoder();
    pair(&mut decoder);
    let queued = output.start(&mono_clip(48_000, &[16_384; 512]), 1.0, &[0], 1.0);
    output.flush();
    drop(decoder);
    assert!(active.finished());
    assert!(queued.finished());
    assert!(output.queue.commands.lock().unwrap().is_empty());
    output.reset_source();
    let fresh = output.start(&mono_clip(48_000, &[16_384; 512]), 1.0, &[0], 1.0);
    output.flush();
    let mut decoder = output.asset().decoder();
    assert_eq!(pair(&mut decoder), gains(0).map(|g| g * 0.5));
    assert!(!fresh.finished());
}

#[test]
fn commands_wait_for_shared_boundary_and_ramp_matches_recovered_block() {
    let mut output = CollisionOutput::default();
    let control = output.start(&mono_clip(48_000, &[16_384; 2048]), 1.0, &[0], 1.0);
    output.flush();
    let mut decoder = output.asset().decoder();
    assert_eq!(decoder.sample_rate(), OUTPUT_RATE);
    assert_eq!(decoder.channels(), 2);
    assert_eq!(decoder.total_duration(), None);
    let old = gains(0).map(|g| g * 0.5);
    assert_eq!(pair(&mut decoder), old);
    output.update(&control, 1.0, &[49_151], 1.0);
    output.flush();
    for _ in 1..FRAMES {
        assert_eq!(pair(&mut decoder), old);
    }
    let target = gains(49_151);
    let ramp =
        std::array::from_fn::<_, 2, _>(|c| pan::block::channel_gains(gains(0)[c], target[c]));
    for frame in 0..FRAMES {
        assert_eq!(
            pair(&mut decoder),
            [ramp[0][frame] * 0.5, ramp[1][frame] * 0.5]
        );
    }
    // Unchanged next block uses the stored matrix, not the ramp's tail.
    assert_eq!(pair(&mut decoder), target.map(|g| g * 0.5));
}

#[test]
fn overlapping_voices_start_and_update_on_one_clock_after_silence() {
    let mut output = CollisionOutput::default();
    let mut decoder = output.asset().decoder();
    skip(&mut decoder, 73); // The clock runs even with no active voices.
    let a = output.start(&mono_clip(48_000, &[16_384; 2048]), 1.0, &[0], 1.0);
    output.flush();
    for _ in 73..FRAMES {
        assert_eq!(pair(&mut decoder), [0.0; 2]);
    }
    let center = gains(0).map(|g| g * 0.5);
    assert_eq!(pair(&mut decoder), center);
    skip(&mut decoder, 90);
    let b = output.start(&mono_clip(48_000, &[8192; 2048]), 1.0, &[0], 1.0);
    output.update(&a, 1.0, &[49_151], 1.0);
    output.flush();
    for _ in 91..FRAMES {
        assert_eq!(pair(&mut decoder), center);
    }
    let ramp = std::array::from_fn::<_, 2, _>(|c| {
        pan::block::channel_gains(gains(0)[c], gains(49_151)[c])
    });
    for frame in 0..FRAMES {
        assert_eq!(
            pair(&mut decoder),
            std::array::from_fn(|c| ramp[c][frame] * 0.5 + gains(0)[c] * 0.25)
        );
    }
    output.update(&a, 1.0, &[0], 1.0);
    output.update(&b, 1.0, &[49_151], 1.0);
    output.flush();
    let a_ramp = std::array::from_fn::<_, 2, _>(|c| {
        pan::block::channel_gains(gains(49_151)[c], gains(0)[c])
    });
    for frame in 0..FRAMES {
        assert_eq!(
            pair(&mut decoder),
            std::array::from_fn(|c| a_ramp[c][frame] * 0.5 + ramp[c][frame] * 0.25)
        );
    }
}

#[test]
fn queued_stops_completion_and_empty_output_keep_the_clock_alive() {
    let mut output = CollisionOutput::default();
    let short = output.start(&mono_clip(48_000, &[16_384, -16_384]), 1.0, &[0], 1.0);
    let stopped = output.start(&mono_clip(48_000, &[16_384; 512]), 1.0, &[0], 1.0);
    output.stop(&stopped);
    output.flush();
    let mut decoder = output.asset().decoder();
    assert_eq!(pair(&mut decoder), gains(0).map(|g| g * 0.5));
    assert!(short.finished());
    assert!(stopped.finished());
    assert_eq!(pair(&mut decoder), gains(0).map(|g| g * -0.5));
    for _ in 2..FRAMES + 8 {
        assert_eq!(pair(&mut decoder), [0.0; 2]);
    }
    assert!(decoder.voices.is_empty());
}

#[test]
fn unpublished_commands_and_stereo_pairs_are_not_partially_applied() {
    let mut output = CollisionOutput::default();
    let control = output.start(&mono_clip(48_000, &[16_384; 1024]), 1.0, &[0], 1.0);
    let mut decoder = output.asset().decoder();
    skip(&mut decoder, FRAMES);
    assert!(decoder.voices.is_empty());
    output.flush();
    assert_eq!(decoder.next().unwrap(), gains(0)[0] * 0.5);
    output.update(&control, 1.0, &[49_151], 0.5);
    output.stop(&control);
    output.flush();
    assert_eq!(decoder.next().unwrap(), gains(0)[1] * 0.5);
    skip(&mut decoder, FRAMES - 1);
    assert!(!control.finished());
    assert_eq!(pair(&mut decoder), [0.0; 2]);
    assert!(control.finished());
}

#[test]
fn ramp_duration_is_independent_of_clip_rate_and_pitch() {
    for rate in [24_000, 48_000] {
        for speed in [0.5, 1.0, 2.0] {
            let mut output = CollisionOutput::default();
            let control = output.start(&mono_clip(rate, &[16_384; 4096]), speed, &[0], 1.0);
            output.flush();
            let mut decoder = output.asset().decoder();
            skip(&mut decoder, FRAMES);
            output.update(&control, speed, &[49_151], 1.0);
            output.flush();
            let ramp = pan::block::channel_gains(gains(0)[0], gains(49_151)[0]);
            for value in ramp {
                assert_eq!(pair(&mut decoder)[0], value * 0.5);
            }
            assert!(!control.finished());
        }
    }
}

#[test]
fn interpolated_pcm_is_panned_after_resampling() {
    let mut samples = [0; 1024];
    samples[128] = 16_384;
    let mut output = CollisionOutput::default();
    let control = output.start(&mono_clip(24_000, &samples), 1.0, &[0], 1.0);
    output.flush();
    let mut decoder = output.asset().decoder();
    skip(&mut decoder, FRAMES);
    output.update(&control, 1.0, &[49_151], 1.0);
    output.flush();
    let ramp = std::array::from_fn::<_, 2, _>(|c| {
        pan::block::channel_gains(gains(0)[c], gains(49_151)[c])
    });
    assert_eq!(pair(&mut decoder), gains(0).map(|g| g * 0.5));
    assert_eq!(pair(&mut decoder), [ramp[0][1] * 0.25, ramp[1][1] * 0.25]);
}

#[test]
fn source_pitch_changes_impulse_time_once_before_shared_panning() {
    for rate in [24_000, 48_000] {
        for speed in [0.5, 1.0, 2.0] {
            let mut samples = [0; 64];
            samples[16] = 16_384;
            let mut output = CollisionOutput::default();
            let control = output.start(&mono_clip(rate, &samples), speed, &[0], 1.0);
            output.flush();
            let mut decoder = output.asset().decoder();
            let frames: Vec<_> = (0..FRAMES).map(|_| pair(&mut decoder)).collect();
            let peak = frames
                .iter()
                .enumerate()
                .max_by(|(_, a), (_, b)| a[0].total_cmp(&b[0]))
                .unwrap()
                .0;
            let expected_peak = (16.0 * OUTPUT_RATE as f32 / (rate as f32 * speed)) as usize;
            assert_eq!(peak, expected_peak);
            assert_eq!(frames[peak], gains(0).map(|g| g * 0.5));
            skip(&mut decoder, FRAMES);
            assert!(control.finished());
            assert_eq!(pair(&mut decoder), [0.0; 2]);
        }
    }
}

#[test]
fn retuning_preserves_fractional_position_and_integrated_duration() {
    for rate in [24_000, 48_000] {
        let samples: Vec<i16> = (0..2048).map(|i| i * 8).collect();
        let mut source = MonoDecoder::new(&mono_clip(rate, &samples), 0.75);
        let mut position = 0.0_f64;
        let mut frames = 0;
        while position < samples.len() as f64 {
            let speed = if frames < FRAMES { 0.75 } else { 1.5 };
            source.retune(speed);
            let actual = source.next().unwrap();
            // A ramp makes cursor resets, duplicate/omitted samples and value
            // discontinuities directly visible at the retuning boundary.
            let expected = position.min(2047.0) as f32 * 8.0 / 32768.0;
            assert_eq!(actual, expected, "{rate} Hz, frame {frames}");
            position += f64::from(rate) / 48000.0 * f64::from(speed);
            frames += 1;
        }
        assert_eq!(source.next(), None);
        let first_distance = FRAMES as f64 * f64::from(rate) / 48000.0 * 0.75;
        let remaining = ((2048.0 - first_distance) / (f64::from(rate) / 48000.0 * 1.5)).ceil();
        assert_eq!(frames, FRAMES + remaining as usize);
    }
}

#[test]
fn pitch_updates_wait_for_boundary_and_latest_start_value_wins() {
    let samples: Vec<i16> = (0..2048).map(|i| i * 8).collect();
    let mut output = CollisionOutput::default();
    let control = output.start(&mono_clip(48_000, &samples), 0.5, &[0], 1.0);
    output.update(&control, 1.0, &[0], 1.0);
    output.flush();
    let mut decoder = output.asset().decoder();
    for frame in 0..FRAMES {
        assert_eq!(
            pair(&mut decoder),
            gains(0).map(|g| g * (frame as f32 / 4096.0))
        );
        if frame == 7 {
            output.update(&control, 0.25, &[0], 1.0);
            output.update(&control, 2.0, &[0], 1.0);
            output.flush();
        }
    }
    for frame in 0..FRAMES {
        let position = FRAMES + 2 * frame;
        assert_eq!(
            pair(&mut decoder),
            gains(0).map(|g| g * (position as f32 / 4096.0))
        );
    }
}

#[test]
fn zero_pitch_holds_cursor_and_voice_until_retuned() {
    let mut output = CollisionOutput::default();
    let control = output.start(&mono_clip(48_000, &[16_384; 512]), 0.0, &[0], 1.0);
    output.flush();
    let mut decoder = output.asset().decoder();
    for _ in 0..FRAMES {
        assert_eq!(pair(&mut decoder), [0.0; 2]);
    }
    assert!(!control.finished());
    output.update(&control, 1.0, &[0], 1.0);
    output.flush();
    assert_eq!(pair(&mut decoder), gains(0).map(|g| g * 0.5));
}
