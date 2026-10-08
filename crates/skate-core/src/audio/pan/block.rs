//! Pn21 changed-matrix stereo block: 82B01210 and 82B13988. The backend
//! processes 256 samples; only the first 64 ramp (822F3480 = 1/64).
//! This operates at the original backend's output rate, after source pitch.
//! Host decoding before rodio resampling is not an equivalent caller.

pub const BLOCK_SAMPLES: usize = 256;
pub const RAMP_SAMPLES: usize = 64;

/// One changed-matrix channel. Preserve the original four-lane starts, the
/// fused 4-sample lane offsets and the fused advance between 32-sample groups.
/// The tail uses the separately calculated endpoint, not `next` itself.
pub fn channel_gains(previous: f32, next: f32) -> [f32; BLOCK_SAMPLES] {
    let step = (next - previous) * (1.0 / RAMP_SAMPLES as f32);
    let four_step = step * 4.0;
    let endpoint = step.mul_add(64.0, previous);
    let mut output = [endpoint; BLOCK_SAMPLES];
    let mut lanes = [
        previous,
        previous + step,
        step.mul_add(2.0, previous),
        step.mul_add(3.0, previous),
    ];
    for chunk in 0..2 {
        for group in 0..8 {
            for lane in 0..4 {
                output[chunk * 32 + group * 4 + lane] = if group == 0 {
                    lanes[lane]
                } else {
                    four_step.mul_add(group as f32, lanes[lane])
                };
            }
        }
        lanes = lanes.map(|value| four_step.mul_add(8.0, value));
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ramp_starts_with_previous_and_holds_after_sixty_four_samples() {
        let gains = channel_gains(0.0, 1.0);
        for (index, value) in gains.iter().enumerate() {
            assert_eq!(*value, index.min(64) as f32 / 64.0);
        }
        let down = channel_gains(1.0, 0.0);
        assert_eq!(down[0], 1.0);
        assert_eq!(down[63], 1.0 / 64.0);
        assert!(down[64..].iter().all(|&gain| gain == 0.0));
    }
    #[test]
    fn owned_block_preserves_fused_lane_order_and_rounded_tail() {
        // Captured from 82B13988 with unity PCM. The endpoint differs from
        // next by two ULPs; substituting a generic lerp loses that behavior.
        let gains = channel_gains(f32::from_bits(0x3f6e331d), f32::from_bits(0x3e4b4f96));
        for (index, bits) in [
            (0, 0x3f6e331d),
            (31, 0x3f1370fe),
            (32, 0x3f108381),
            (63, 0x3e570586),
            (64, 0x3e4b4f94),
            (255, 0x3e4b4f94),
        ] {
            assert_eq!(gains[index].to_bits(), bits);
        }
    }

    #[test]
    fn unchanged_matrix_has_no_ramp_or_drift() {
        for value in [0.0, 0.70710677, 1.0] {
            assert_eq!(channel_gains(value, value), [value; BLOCK_SAMPLES]);
        }
    }
}
