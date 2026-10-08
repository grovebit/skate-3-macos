//! MixMap scalar declaration construction and evaluation, base-disc
//! 8292A89C..8292AD3C and 82928140..829283A0. Tables and resolved input values
//! come from the owned program; this module does not infer live input bindings.

pub struct ScalarTables {
    log: [i32; 512],
    volume: [i32; 602],
}

impl ScalarTables {
    /// Enabled ordinary format-0 output conversion (82928658/829288CC).
    /// Preserves word overflow before clamping, and returns the packed level
    /// consumed by the collision voice's volume accessor.
    pub fn format0_volume(&self, record: i32, adjustment: i16, modulation: Option<i32>) -> u16 {
        let level = record
            .wrapping_add(modulation.unwrap_or(0))
            .wrapping_add(i32::from(adjustment))
            .clamp(-10000, 0);
        self.attenuation((-level) as usize) as u16
    }

    fn attenuation(&self, magnitude: usize) -> i32 {
        let shift = magnitude / 602;
        if shift > 15 {
            0
        } else {
            self.volume[601 - magnitude % 602] >> shift
        }
    }
    /// Owned big-endian tables at 82FBBD98 and 82FBB430 respectively.
    pub fn from_be_bytes(log: &[u8], volume: &[u8]) -> Option<Self> {
        Some(Self {
            log: super::read_level_table(log, 601)?,
            volume: super::read_level_table(volume, 32767)?,
        })
    }

    /// Native logarithmic conversion; sparse low bins select their upper end.
    pub fn linear_to_log(&self, level: u16) -> Option<i32> {
        if level > 32767 {
            return None;
        }
        if level == 0 {
            return Some(-10000);
        }
        let exponent = 15 - level.leading_zeros() as usize;
        let remainder = usize::from(level) - (1 << exponent);
        let index = if exponent >= 9 {
            remainder >> (exponent - 9)
        } else {
            let shift = 9 - exponent;
            (remainder << shift) + (1 << shift) - 1
        };
        Some(self.log[index] - 602 * (15 - exponent as i32))
    }

    /// Ratio to cents, 82923820..82923D80. Both branches use the same
    /// logarithm bins; the reciprocal branch has the negative multiplier.
    pub fn ratio_cents(&self, ratio: f32) -> Option<i32> {
        if !ratio.is_finite() || ratio < 0.0 {
            return None;
        }
        let reciprocal = ratio > 1.0;
        let linear = if reciprocal {
            32767.0 / ratio
        } else {
            ratio * 32767.0
        };
        let log = self.linear_to_log(linear as u16)?;
        let scale = f32::from_bits(0x3fff1fc4); // 822F3784; negative at 822F3780.
        Some((log as f32 * if reciprocal { -scale } else { scale }) as i32)
    }

    /// Construct metadata +08/+0C from the descriptor's signed low halfword.
    /// Both signs use the magnitude for the table conversion; only positive
    /// authored values also contribute an additive base.
    pub fn declaration(&self, authored: i16) -> ScalarDeclaration {
        let magnitude = i32::from(authored).unsigned_abs() as usize;
        let converted = self.attenuation(magnitude);
        ScalarDeclaration {
            base: i32::from(authored).max(0),
            depth: 32767 - converted,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ScalarDeclaration {
    pub(super) base: i32,
    pub(super) depth: i32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ScalarValue {
    pub shaped_linear: u16,
    pub log_level: i32,
    pub gate_weight: i32,
    pub level: i32,
}

impl ScalarDeclaration {
    /// `source_linear` is the resolved curve output. Gates are resolved linear
    /// references in authored binding order, not logarithmic output levels.
    /// A null gate array leaves the initial 32767 weight. A present array uses
    /// only the low byte of its expanded count, matching the rewritten header.
    pub fn evaluate(
        &self,
        source_linear: u16,
        gates: Option<&[u16]>,
        tables: &ScalarTables,
    ) -> Option<ScalarValue> {
        if source_linear > 32767 {
            return None;
        }
        let shaped = 32767 - (((32767 - i32::from(source_linear)) * self.depth) >> 15);
        let log_level = tables.linear_to_log(shaped as u16)?;
        let gate_weight = match gates {
            Some(gates) => super::control_weight(gates)?,
            None => 32767,
        };
        let level = gate_weight.wrapping_mul(self.base.wrapping_add(log_level)) >> 15;
        Some(ScalarValue {
            shaped_linear: shaped as u16,
            log_level,
            gate_weight,
            level,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn tables() -> ScalarTables {
        ScalarTables {
            log: std::array::from_fn(|i| i as i32),
            volume: [32000; 602],
        }
    }

    #[test]
    fn scalar_output_and_volume_feed_the_voice_consumer_without_float_gain_substitution() {
        let tables = tables();
        let scalar = tables
            .declaration(1000)
            .evaluate(32767, None, &tables)
            .unwrap();
        let output = crate::audio::output::output_level(0, true, Some(&[scalar.level]));
        // 908 - 1100 = -192; synthetic table deliberately uses 32000.
        let packed = tables.format0_volume(output, -1100, None);
        assert_eq!(packed, 32000);
        let voice =
            crate::audio::voice::collision_gain(32767, 32767, Some(&[u32::from(packed)]), 0)
                .unwrap();
        assert_eq!(voice.after_controller, 32000);
        assert_eq!(voice.after_material, 32000);
        assert_eq!(tables.format0_volume(i32::MAX, 1, None), 0);
        assert_eq!(tables.format0_volume(-9632, 0, None), 0);
        assert_eq!(tables.format0_volume(0, 0, Some(-602)), 16000);
    }

    #[test]
    fn construction_preserves_sign_shift_boundaries_and_cutoff() {
        let tables = tables();
        for (authored, depth) in [
            (0, 767),
            (601, 767),
            (602, 16767),
            (9030, 32767),
            (9632, 32767),
            (32767, 32767),
        ] {
            assert_eq!(
                tables.declaration(authored),
                ScalarDeclaration {
                    base: i32::from(authored),
                    depth
                }
            );
            assert_eq!(
                tables.declaration(-authored),
                ScalarDeclaration { base: 0, depth }
            );
        }
        assert_eq!(
            tables.declaration(i16::MIN),
            ScalarDeclaration {
                base: 0,
                depth: 32767
            }
        );
    }

    #[test]
    fn logarithm_uses_sparse_bin_upper_ends_and_zero_sentinel() {
        let tables = tables();
        assert_eq!(tables.linear_to_log(0), Some(-10000));
        assert_eq!(tables.linear_to_log(1), Some(511 - 602 * 15));
        assert_eq!(tables.linear_to_log(2), Some(255 - 602 * 14));
        assert_eq!(tables.linear_to_log(512), Some(-602 * 6));
        assert_eq!(tables.linear_to_log(32767), Some(511 - 602));
        assert_eq!(tables.linear_to_log(32768), None);
    }

    #[test]
    fn gate_order_and_fixed_point_rounding_are_not_a_float_product() {
        let tables = tables();
        let d = tables.declaration(1000);
        let ungated = d.evaluate(32767, None, &tables).unwrap();
        assert_eq!(ungated.shaped_linear, 32767);
        assert_eq!(ungated.log_level, -91);
        assert_eq!(ungated.level, 908); // 32767 is not 32768.
        let gated = d.evaluate(32767, Some(&[32767, 16384]), &tables).unwrap();
        assert_eq!(gated.gate_weight, 16383);
        assert_eq!(gated.level, 454);
        assert_eq!(d.evaluate(0, Some(&[0]), &tables).unwrap().level, 0);
        assert_eq!(d.evaluate(0, Some(&[32768]), &tables), None);
        assert_eq!(d.evaluate(32768, None, &tables), None);
        let gates = [0; 257];
        assert_eq!(
            d.evaluate(32767, Some(&gates[..256]), &tables).unwrap(),
            ungated
        );
        assert_eq!(d.evaluate(32767, Some(&gates), &tables).unwrap().level, 0);
    }

    #[test]
    fn table_reader_rejects_wrong_size_and_out_of_range_words() {
        let bytes = |n, value: i32| (0..n).flat_map(|_| value.to_be_bytes()).collect::<Vec<_>>();
        assert!(ScalarTables::from_be_bytes(&bytes(512, 0), &bytes(602, 32767)).is_some());
        assert!(ScalarTables::from_be_bytes(&bytes(511, 0), &bytes(602, 0)).is_none());
        assert!(ScalarTables::from_be_bytes(&bytes(512, 602), &bytes(602, 0)).is_none());
        assert!(ScalarTables::from_be_bytes(&bytes(512, 0), &bytes(602, -1)).is_none());
    }
}
