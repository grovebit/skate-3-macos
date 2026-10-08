//! SFXCTL_PlayerPhysics publication 8249ECA0 (registration 82FD11D0, vtable
//! 822F7CE4 slot +24): the speed graph on the body contact block. Base-disc
//! default.xex SHA-256
//! 1db39496585c521d17a2137804f42cf73ebed2b32cac166ec42dbf772f4dcf7f.
//!
//! The publisher copies the player record's 0x74-byte body block to snapshot
//! +1F0 (8249F178..8249F198), then multiplies all eight region strengths by
//! `aud_collisions/default` `Hash_8B164823E008749C` evaluated at the previous
//! call's COM speed (8249F794..8249F7D4). Nothing clamps the products. The
//! Player group updates its controllers before its objects (828B7C58), so
//! the Contacts body loop reads them in the same input update.
use super::collision_position::length;
use crate::{physics::native_arithmetic::dot3, point_graph::PointGraph};

/// Snapshot +1F0 regions: head, torso, left and right arm, left and right
/// leg, then the two feet.
pub const BLOCK_REGIONS: usize = 8;

/// Record +6C, written to packet +6C by 8277E898..8277E8F0: the three-lane
/// length of PhysOut_SystemReckoning +10, the skeleton COM velocity that
/// 82BB18D8 derives over the physics step (world units per second). The
/// final sign clear cannot change this non-negative length.
pub fn com_speed(velocity: [f32; 4]) -> f32 {
    length(dot3(velocity, velocity))
}

/// Snapshot +D8, the previous call's +D4. Initializer 8249E968 zeroes both;
/// the constructor and reset virtual +2C (8249EC98) run it.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PlayerPhysics {
    previous_speed: f32,
}

impl PlayerPhysics {
    /// One publication: +D4 takes `speed` (8249EE28), each strength is
    /// multiplied by the graph at +D8 (8246FD40, `fmuls` at 8249F7C0), then
    /// +D8 takes +D4 (8249F7D0). The loop calls the graph once per region
    /// with unchanged inputs; one evaluation yields the same products.
    pub fn publish(
        &mut self,
        strengths: [f32; BLOCK_REGIONS],
        speed: f32,
        graph: &PointGraph<8>,
    ) -> [f32; BLOCK_REGIONS] {
        let gain = graph.evaluate(self.previous_speed);
        self.previous_speed = speed;
        strengths.map(|strength| gain * strength)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `aud_collisions/default` `Hash_8B164823E008749C` words +10..+4F.
    fn owned() -> PointGraph<8> {
        PointGraph {
            x: [
                0x0000_0000,
                0x3EBB_9F41,
                0x3EE5_50DE,
                0x3F05_6B91,
                0x3F17_C3F8,
                0x3F3D_B4F8,
                0x3F5C_FA27,
                0x3F72_3DB4,
            ]
            .map(f32::from_bits),
            y: [
                0x3F80_0000,
                0x3F99_999A,
                0x3FBE_2BE0,
                0x3FF5_0753,
                0x401B_6DB5,
                0x4066_6666,
                0x4092_4921,
                0x40A0_0000,
            ]
            .map(f32::from_bits),
        }
    }

    #[test]
    fn the_owned_graph_matches_native_8246fd40_values() {
        let graph = owned();
        // Input bits -> native result bits from the local 8246FD40 replay.
        for (input, expected) in [
            (0xBF80_0000, 0x3F80_0000), // below the first x
            (0x0000_0000, 0x3F80_0000), // the first x
            (0x3E80_0000, 0x3F91_7704), // 0.25, first piece
            (0x3F00_0000, 0x3FE5_2E58), // 0.5, between x2 and x3
            (0x3F72_3DB3, 0x409F_FFFF), // just below the last x
            (0x3F72_3DB4, 0x40A0_0000), // the last x
            (0x40F0_0000, 0x40A0_0000), // 7.5, above the last x
            (0x7FC0_0000, 0x40A0_0000), // unordered
        ] {
            assert_eq!(
                graph.evaluate(f32::from_bits(input)).to_bits(),
                expected,
                "{input:08x}"
            );
        }
    }

    #[test]
    fn the_previous_publication_speed_scales_all_eight_regions_unclamped() {
        let graph = owned();
        let mut physics = PlayerPhysics::default();
        let strengths = [0.9, 0.5, 0.25, 0.0, 1.0, 0.001, 0.75, 0.2];
        // The first publication after construction or reset reads +D8 = 0.
        assert_eq!(physics.publish(strengths, 7.5, &graph), strengths);
        let fast = physics.publish(strengths, 0.5, &graph);
        assert_eq!(fast, strengths.map(|s| 5.0 * s));
        assert_eq!(fast[0], 4.5);
        let between = physics.publish(strengths, 0.0, &graph);
        let gain = f32::from_bits(0x3FE5_2E58);
        assert_eq!(between, strengths.map(|s| gain * s));
        assert_eq!(physics.publish(strengths, 0.0, &graph), strengths);
        physics = PlayerPhysics::default();
        assert_eq!(physics.publish(strengths, 0.0, &graph), strengths);
    }

    #[test]
    fn speed_is_the_three_lane_length() {
        assert_eq!(com_speed([0.0, -0.0, 0.0, 9.0]), 0.0);
        let speed = com_speed([3.0, 4.0, 0.0, 100.0]);
        assert!((speed - 5.0).abs() <= 5.0 * f32::EPSILON, "{speed}");
    }
}
