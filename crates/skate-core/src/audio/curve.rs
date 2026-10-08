//! MixMap curve stage: 82923F18..82924100, float wrapper 82923EA8,
//! and shared curve values updated by 82927F04..82928118.
use super::scalar::ScalarTables;

pub struct CurveTable([i32; 513]);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CurveValue {
    pub linear: u16,
    pub log: i32,
}
impl Default for CurveValue {
    /// Shared input node initialization at 829266A8..829266B0.
    fn default() -> Self {
        Self {
            linear: 32767,
            log: 0,
        }
    }
}

impl CurveTable {
    /// Owned big-endian table at 82FBC598. No generated approximation.
    pub fn from_be_bytes(bytes: &[u8]) -> Option<Self> {
        Some(Self(super::read_level_table(bytes, 32767)?))
    }

    /// The evaluator checks selectors before dereferencing the source; unknown
    /// selectors produce zero. Valid selectors require a 15-bit source value.
    pub fn evaluate(&self, source: u16, selector: u8) -> Option<u16> {
        if selector > 9 {
            return Some(0);
        }
        if source > 32767 {
            return None;
        }
        Some(self.run(i32::from(source), selector) as u16)
    }

    pub fn evaluate_float(&self, source: f32, selector: u32) -> Option<f32> {
        if selector > 9 {
            return Some(0.);
        }
        if !source.is_finite() || !(0.0..=1.0).contains(&source) {
            return None;
        }
        let level = self.evaluate((source * 32767.) as u16, selector as u8)?;
        Some(f32::from(level) * f32::from_bits(0x38000100))
    }

    /// Resolve both fields of the shared curve node. Declaration source/gate
    /// bindings consume linear; other references may consume log separately.
    pub fn value(&self, source: u16, selector: u8, tables: &ScalarTables) -> Option<CurveValue> {
        let linear = self.evaluate(source, selector)?;
        Some(CurveValue {
            linear,
            log: tables.linear_to_log(linear)?,
        })
    }

    fn run(&self, source: i32, selector: u8) -> i32 {
        if selector & 1 != 0 {
            return self.run(32767 - source, selector - 1);
        }
        if selector == 8 {
            return 32767 - source;
        }
        if matches!(selector, 2 | 6) {
            let value = self.run(source, selector - 2);
            return value * value >> 15;
        }
        let weight = 0x3ff | ((source << 9) & 0x3c00);
        let index = (source >> 6) as usize;
        let (first, second) = if selector == 0 {
            if index >= 511 || self.0[index] == 0 {
                return 0;
            }
            (self.0[index], self.0[index + 1])
        } else {
            let index = 511 - index;
            (32767 - self.0[index], 32767 - self.0[index + 1])
        };
        first + (((second - first) * weight) >> 15)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn table() -> CurveTable {
        CurveTable(std::array::from_fn(|i| (32767 - i as i32 * 64).max(0)))
    }
    #[test]
    fn complements_squares_and_endpoint_guards_preserve_native_rules() {
        let table = table();
        for source in [0, 1, 31, 32, 63, 64, 16384, 32766, 32767] {
            for even in [0, 2, 4, 6, 8] {
                assert_eq!(
                    table.evaluate(source, even + 1),
                    table.evaluate(32767 - source, even)
                );
            }
            for (square, base) in [(2, 0), (6, 4)] {
                let value = i32::from(table.evaluate(source, base).unwrap());
                assert_eq!(
                    table.evaluate(source, square),
                    Some((value * value >> 15) as u16)
                );
            }
            assert_eq!(table.evaluate(source, 8), Some(32767 - source));
            assert_eq!(table.evaluate(source, 9), Some(source));
        }
        assert_eq!(table.evaluate(32767, 0), Some(0));
        assert_eq!(table.evaluate(32768, 8), None);
        assert_eq!(table.evaluate(65535, 10), Some(0));
    }
    #[test]
    fn interpolation_weight_uses_only_the_original_inserted_bits() {
        let table = table();
        assert_eq!(table.evaluate(0, 0), Some(32765));
        assert_eq!(table.evaluate(1, 0), Some(32765));
        assert_eq!(table.evaluate(2, 0), Some(32763));
        assert_eq!(table.evaluate(31, 0), Some(32735));
        assert_eq!(table.evaluate(32, 0), Some(32765));
    }
    #[test]
    fn float_wrapper_checks_domain_after_invalid_selector_bypass() {
        let table = table();
        assert_eq!(table.evaluate_float(f32::NAN, 10), Some(0.));
        assert_eq!(table.evaluate_float(f32::NAN, 9), None);
        assert_eq!(table.evaluate_float(1.001, 9), None);
        assert_eq!(table.evaluate_float(1., 9), Some(1.));
        assert_eq!(table.evaluate_float(0., 9), Some(0.));
    }
    #[test]
    fn curve_fields_feed_scalar_and_output_without_confusing_linear_and_log() {
        let bytes = |count, value: i32| {
            (0..count)
                .flat_map(|_| value.to_be_bytes())
                .collect::<Vec<_>>()
        };
        let tables = ScalarTables::from_be_bytes(&bytes(512, 601), &bytes(602, 32730)).unwrap();
        let curve = table().value(32767, 9, &tables).unwrap();
        assert_eq!(
            curve,
            CurveValue {
                linear: 32767,
                log: -1
            }
        );
        let value = tables
            .declaration(555)
            .evaluate(curve.linear, None, &tables)
            .unwrap();
        assert_eq!(value.level, 553);
        assert_eq!(
            crate::audio::output::output_level(0, true, Some(&[value.level])),
            553
        );
        assert_eq!(
            CurveValue::default(),
            CurveValue {
                linear: 32767,
                log: 0
            }
        );
        assert!(CurveTable::from_be_bytes(&bytes(512, 0)).is_none());
        assert!(CurveTable::from_be_bytes(&bytes(513, -1)).is_none());
        assert!(CurveTable::from_be_bytes(&bytes(513, 32767)).is_some());
    }
}
