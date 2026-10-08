//! Shared sums, base-disc binding 8292B928 and evaluation 829283DC..82928458.
//! Inputs are resolved signed levels, not the linear values used by curve gates.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SumError {
    InvalidProgram,
    InvalidInstances,
    InvalidInstance,
    MissingCount(u8),
    InvalidCount(u8),
    MissingInput(u32),
}

/// Shared sums and non-modulation output arguments use this expansion rule.
/// Counts are the original factory counts, including explicit zero counts for
/// absent families. Preserve duplicates and authored order. Scalar gates have
/// a different rule and must not use this helper.
pub fn expand_references(
    references: &[u32],
    family: u8,
    instance: u8,
    counts: &[u8],
) -> Result<Vec<u32>, SumError> {
    if instance >= 32 {
        return Err(SumError::InvalidInstance);
    }
    let mut result = Vec::new();
    for &reference in references {
        let target = (reference >> 16) as u8;
        let base = reference & 0xffff07ff;
        if target == family {
            result.push(base | (u32::from(instance) << 11));
        } else {
            let count = *counts
                .get(target as usize)
                .ok_or(SumError::MissingCount(target))?;
            if count > 32 {
                return Err(SumError::InvalidCount(target));
            }
            result.extend((0..count).map(|i| base | (u32::from(i) << 11)));
        }
    }
    Ok(result)
}

pub struct SharedSum {
    references: Option<Vec<u32>>,
    clamp: u32,
    level: i32,
}
pub mod mxb;
impl SharedSum {
    /// Accept already expanded references and explicit prior state. `None`
    /// represents a null runtime array, which retains its previous level.
    /// A present empty array recomputes zero. No lifecycle/reset is inferred.
    pub fn new(references: Option<Vec<u32>>, clamp: u32, previous: i32) -> Self {
        Self {
            references,
            clamp,
            level: previous,
        }
    }

    pub fn level(&self) -> i32 {
        self.level
    }

    pub fn references(&self) -> Option<&[u32]> {
        self.references.as_deref()
    }

    /// Run after the referenced producers, in original sum order. Resolve
    /// only the low byte of the expanded count. Missing inputs fail without
    /// replacing the prior level; they are not silent zero-valued controls.
    pub fn advance(
        &mut self,
        mut resolve: impl FnMut(u32) -> Option<i32>,
    ) -> Result<i32, SumError> {
        let Some(references) = &self.references else {
            return Ok(self.level);
        };
        let mut total = 0_i32;
        for &reference in &references[..references.len() & 255] {
            total =
                total.wrapping_add(resolve(reference).ok_or(SumError::MissingInput(reference))?);
        }
        let upper = ((self.clamp >> 16) & 0x7fff) as i32;
        // Native OR with FFFF0000 forces the lower bound negative, even
        // when bit 15 is clear. This is not an i16 sign extension.
        let lower = (self.clamp & 0xffff) as i32 - 0x10000;
        self.level = total.clamp(lower, upper);
        Ok(self.level)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn binding_clears_instance_bits_and_expands_external_families_in_order() {
        assert_eq!(
            expand_references(
                &[0x30033000, 0xb100304c, 0xb1013014, 0xb100304c],
                3,
                2,
                &[2, 0]
            )
            .unwrap(),
            [0x30031000, 0xb100004c, 0xb100084c, 0xb100004c, 0xb100084c]
        );
        assert_eq!(
            expand_references(&[0xb100304c], 3, 0, &[]),
            Err(SumError::MissingCount(0))
        );
        assert_eq!(
            expand_references(&[0xb100304c], 3, 0, &[33]),
            Err(SumError::InvalidCount(0))
        );
        assert_eq!(
            expand_references(&[], 3, 32, &[]),
            Err(SumError::InvalidInstance)
        );
    }
    #[test]
    fn state_distinguishes_null_empty_and_missing_inputs() {
        let mut sum = SharedSum::new(None, 0x0000d8f0, -321);
        assert_eq!(
            sum.advance(|_| panic!("null array must not resolve")),
            Ok(-321)
        );
        let mut sum = SharedSum::new(Some(vec![]), 0x0000d8f0, -321);
        assert_eq!(sum.advance(|_| None), Ok(0));
        let mut sum = SharedSum::new(Some(vec![1, 2]), 0x0000d8f0, -321);
        assert_eq!(
            sum.advance(|r| (r == 1).then_some(-100)),
            Err(SumError::MissingInput(2))
        );
        assert_eq!(sum.level(), -321);
    }
    #[test]
    fn wrapping_clamp_and_low_byte_count_match_native() {
        let mut sum = SharedSum::new(Some(vec![0, 1]), 0x0000d8f0, 0);
        assert_eq!(sum.advance(|i| Some([i32::MAX, 1][i as usize])), Ok(-10000));
        assert_eq!(sum.advance(|i| Some([i32::MIN, -1][i as usize])), Ok(0));
        let mut sum = SharedSum::new(Some(vec![0]), 0x800a0001, 0);
        assert_eq!(sum.advance(|_| Some(-100000)), Ok(-65535));
        assert_eq!(sum.advance(|_| Some(100)), Ok(10));
        for count in [255, 256, 257] {
            let mut sum = SharedSum::new(Some(vec![0; count]), 0x0000d8f0, -999);
            let mut calls = 0;
            assert_eq!(
                sum.advance(|_| {
                    calls += 1;
                    Some(-1)
                }),
                Ok(-((count & 255) as i32))
            );
            assert_eq!(calls, count & 255);
        }
    }
}
