//! SFXCTL_3DColPos input publication for one collision group: listener
//! update 8247AA48, publication 8249CAD8, frame 8249C988, distances 8249C540,
//! phase 8249BFD8 and angle 82441210. Base-disc default.xex SHA-256
//! 1db39496585c521d17a2137804f42cf73ebed2b32cac166ec42dbf772f4dcf7f.
//!
//! VMX estimates use the port's working PC math (`native_arithmetic`) with
//! the recovered refinement steps, as elsewhere in the port; results can
//! differ from Xenon in the last bits, not in the published formulas.
use crate::physics::native_arithmetic::{
    dot3, reciprocal_estimate, reciprocal_square_root_estimate,
};

/// Listener root fields +00 (position) and +20 (normalized "at").
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Listener {
    pub position: [f32; 4],
    pub at: [f32; 4],
}

/// `vrsqrtefp` followed by two Newton steps, as in 8247AAF4..8247AB14.
fn refined_rsqrt(value: f32) -> f32 {
    let mut inverse = reciprocal_square_root_estimate(value);
    for _ in 0..2 {
        inverse = (inverse * 0.5).mul_add((-value).mul_add(inverse * inverse, 1.0), inverse);
    }
    inverse
}

/// `vrefp` followed by two Newton steps (8249C108..8249C190).
fn refined_reciprocal(value: f32) -> f32 {
    let mut inverse = reciprocal_estimate(value);
    for _ in 0..2 {
        inverse = inverse.mul_add((-inverse).mul_add(value, 1.0), inverse);
    }
    inverse
}

/// Length from the refined estimate; a zero square selects zero.
fn length(square: f32) -> f32 {
    if square == 0.0 {
        0.0
    } else {
        square * refined_rsqrt(square)
    }
}

impl Listener {
    /// 8247AA48: the active camera record's row +30 is the position and row
    /// +20 ("at") is scaled by the refined reciprocal square root of its
    /// three-component dot product.
    pub fn from_camera(position: [f32; 4], at: [f32; 4]) -> Self {
        let scale = refined_rsqrt(dot3(at, at));
        Self {
            position,
            at: at.map(|v| v * scale),
        }
    }
}

/// Scalar lane of 82441210: acos from the twelve coefficients at
/// 822F4480..822F44AC, guard 822FABB0 (1.0000001) and pi 822F44B0.
pub fn acos(x: f32) -> f32 {
    let c = |bits: u32| f32::from_bits(bits);
    let a = x.abs();
    let cube = (x * x) * a;
    let tail = (-a).mul_add(x, x); // x - |x|x
    let v5 = c(0x400b1889)
        .mul_add(a, c(0xc0d1360e))
        .mul_add(a, c(0x40af6ad8));
    let v6 = c(0x3e663246)
        .mul_add(a, c(0xbf983f2f))
        .mul_add(a, c(0x3fb58485));
    let v7 = c(0xbed65553)
        .mul_add(a, c(0x408980bd))
        .mul_add(a, c(0xc08f6ad9));
    let v9 = c(0xbd6dd42d)
        .mul_add(a, c(0x3f1dd7b6))
        .mul_add(a, c(0xbfaf4418));
    let near = v6.mul_add(cube, v5);
    let far = v9.mul_add(cube, v7);
    let gap = c(0x3f800001) - a;
    let estimate = reciprocal_square_root_estimate(gap);
    let step = (-(gap * 0.5)).mul_add(estimate * estimate, 0.5);
    let root = estimate.mul_add(step, estimate);
    let sum = (tail * far).mul_add(root, x * near);
    c(0x40490fdb) * 0.5 - sum
}

/// 8249BFD8: oriented angle between the projected forward `f` and the
/// projected source offset from `origin`, as a 16-bit turn. Zero when
/// either length is below 1e-4 (8209BE30) or unordered.
pub fn phase(f: [f32; 2], origin: [f32; 2], source: [f32; 2]) -> u32 {
    let d = [source[0] - origin[0], source[1] - origin[1]];
    let f_square = (f[0] * f[0]) + (f[1] * f[1]);
    let d_square = (d[0] * d[0]) + (d[1] * d[1]);
    let f_length = length(f_square);
    let d_length = length(d_square);
    let threshold = f32::from_bits(0x38d1b717);
    if !(d_length >= threshold) || !(f_length >= threshold) {
        return 0;
    }
    let d_scale = refined_reciprocal(d_length);
    let f_scale = refined_reciprocal(f_length);
    let dn = [d_scale * d[0], d_scale * d[1]];
    let fn_ = [f_scale * f[0], f_scale * f[1]];
    let dot = (dn[0] * fn_[0]) + (dn[1] * fn_[1]);
    let clamped = (-1.0_f32).max(dot).min(1.0);
    let turns = (acos(clamped) * 65535.0) * f32::from_bits(0x3e22f983);
    let mut result = turns as i32 as u32;
    let cross = (dn[0] * fn_[1]) - (dn[1] * fn_[0]);
    if cross > 0.0 {
        result = 0xffff_u32.wrapping_sub(result);
    }
    result
}

/// One 3DColPos input publication. `source` is the queued record's +10
/// copy, present while the group is active (activation 824BFC58, reset
/// 8249CAC8). `offset` is aud_general/camera Hash_A6F853B935E46E5F at
/// controller +70. Words 0 and 2 (second listener, root +40 from
/// context +29070 +08) are not published here: the collision modulation
/// nodes select words 1 and 3.
pub fn publish(words: &mut [u32; 16], source: Option<[f32; 4]>, listener: &Listener, offset: f32) {
    let Some(source) = source else {
        // 8249CB00..8249CC00: 3 and 2 integer zero, 1 and 0 float -1.0,
        // then word 15 with bit 0 cleared.
        words[3] = 0;
        words[1] = (-1.0_f32).to_bits();
        words[2] = 0;
        words[0] = (-1.0_f32).to_bits();
        words[15] &= !1;
        return;
    };
    words[15] |= 1;
    // Controller +38 is published, then zeroed by the frame update.
    words[11] = 0;
    // 8249C988: X/Z projection (permute 822F6370), origin pulled back
    // along the normalized projected forward by `offset`.
    let at = [listener.at[0], listener.at[2]];
    let at_square = (at[0] * at[0]) + (at[1] * at[1]);
    let scale = refined_rsqrt(at_square);
    let origin = [
        listener.position[0] - (at[0] * scale) * offset,
        listener.position[2] - (at[1] * scale) * offset,
    ];
    words[3] = phase(at, origin, [source[0], source[2]]);
    let delta = core::array::from_fn(|i| listener.position[i] - source[i]);
    words[1] = length(dot3(delta, delta)).to_bits();
    // Additional source +1C is never set for collisions.
    words[5] = 0;
    words[6] = 0;
    words[10] = 0;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn angle_polynomial_tracks_acos_across_the_domain() {
        for i in -1000..=1000 {
            let x = i as f32 / 1000.0;
            let error = (acos(x) - x.acos()).abs();
            assert!(error < 2e-5, "{x}: {} vs {}", acos(x), x.acos());
        }
    }

    #[test]
    fn phase_is_zero_ahead_and_mirrored_by_side() {
        let ahead = phase([0.0, 1.0], [0.0, 0.0], [0.0, 5.0]);
        assert_eq!(ahead, 0);
        let behind = phase([0.0, 1.0], [0.0, 0.0], [0.0, -5.0]);
        assert!((32760..=32767).contains(&behind), "{behind}");
        let left = phase([0.0, 1.0], [0.0, 0.0], [5.0, 0.0]);
        let right = phase([0.0, 1.0], [0.0, 0.0], [-5.0, 0.0]);
        assert!((16380..=16390).contains(&left.min(right)));
        assert_eq!(left + right, 0xffff);
        // Degenerate lengths publish zero rather than an arbitrary angle.
        assert_eq!(phase([0.0, 0.0], [0.0, 0.0], [5.0, 0.0]), 0);
        assert_eq!(phase([0.0, 1.0], [1.0, 1.0], [1.0, 1.00001]), 0);
    }

    #[test]
    fn publication_sets_distance_phase_and_enable_and_clears_without_source() {
        let listener = Listener::from_camera([0.0, 1.5, 0.0, 1.0], [0.0, -0.5, 2.0, 0.0]);
        assert!((listener.at[2] - 0.970_142_5).abs() < 1e-6);
        let mut words = [0; 16];
        words[15] = 6;
        publish(&mut words, Some([3.0, 1.5, 4.0, 1.0]), &listener, 0.25);
        assert_eq!(words[15], 7);
        assert!((f32::from_bits(words[1]) - 5.0).abs() < 1e-5);
        // The origin sits 0.25 behind the listener, so the offset (3, 4.25)
        // is 6411 turns; +X of a +Z forward mirrors (8249C228).
        assert!(
            (65535 - 6413..=65535 - 6409).contains(&words[3]),
            "{}",
            words[3]
        );
        publish(&mut words, None, &listener, 0.25);
        assert_eq!(words[15], 6);
        assert_eq!(
            (words[0], words[1], words[2], words[3]),
            ((-1.0_f32).to_bits(), (-1.0_f32).to_bits(), 0, 0)
        );
        // A coincident source has zero distance, not 0 * inf.
        publish(&mut words, Some(listener.position), &listener, 0.25);
        assert_eq!(f32::from_bits(words[1]), 0.0);
    }
}
