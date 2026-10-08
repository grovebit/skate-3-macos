//! Collision pan consumer 824C0370..824C0414 and Pn21 mono-to-stereo
//! matrix 82AFE298, 82B1D930..82B1D9C8. Base-disc default.xex SHA-256
//! 1db39496585c521d17a2137804f42cf73ebed2b32cac166ec42dbf772f4dcf7f.
//! Host trigonometry/sqrt substitute for the original runtime math helpers.
pub mod block;

use super::voice::{VoiceError, controller_word};

/// Virtual +34 reads all sixteen bits (824B9D10), unlike volume +3C.
/// The output phase supplies degrees to AEMS parameter +08.
pub fn collision_degrees(words: Option<&[u32]>) -> Result<f32, VoiceError> {
    Ok(controller_word(words, 0)? as f32 * f32::from_bits(0x3bb400b4))
}

/// Pn21's mono source at unit radius, unit gain and stereo output. The
/// stereo branch uses the position's Y component; front/back both centre.
/// AEMS layer angle offsets, radius and block smoothing belong to the layer
/// backend and must be supplied separately when that backend is ported.
pub fn mono_stereo(degrees: f32) -> [f32; 2] {
    let radians = degrees * f32::from_bits(0xbc8efa35);
    let x = radians.cos();
    let mut y = radians.sin();
    // 82B1CC68 normalizes vectors whose rounded square exceeds one.
    let square = x.mul_add(x, y * y);
    if square > 1.0 {
        let inverse = 1.0 / square.sqrt();
        y *= inverse;
    }
    let left = (y + 1.0) * 0.5;
    let right = 1.0 - left;
    let norm = 1.0 / right.mul_add(right, left * left).sqrt();
    [norm * left, right * norm]
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn pan_keeps_high_bit_and_null_owner_is_ahead() {
        assert_eq!(collision_degrees(None), Ok(0.0));
        assert_eq!(collision_degrees(Some(&[0xabcdffff])), Ok(360.0));
        assert_eq!(
            collision_degrees(Some(&[])),
            Err(VoiceError::IncompleteController)
        );
        assert!(collision_degrees(Some(&[0x8000])).unwrap() > 180.0);
    }
    #[test]
    fn stereo_cardinals_and_turn_seam_preserve_power() {
        for (angle, expected) in [
            (0., [0.70710677, 0.70710677]),
            (90., [0., 1.]),
            (180., [0.70710677, 0.70710677]),
            (270., [1., 0.]),
            (360., [0.70710677, 0.70710677]),
        ] {
            let actual = mono_stereo(angle);
            for i in 0..2 {
                assert!(
                    (actual[i] - expected[i]).abs() < 1e-6,
                    "{angle}: {actual:?}"
                );
            }
        }
        for phase in 0..=65535 {
            let gains = mono_stereo(collision_degrees(Some(&[phase])).unwrap());
            assert!((gains[0] * gains[0] + gains[1] * gains[1] - 1.).abs() < 3e-7);
        }
    }
}
