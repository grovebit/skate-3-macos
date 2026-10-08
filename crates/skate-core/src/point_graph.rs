//! `PointGraph` scalar evaluator: base-disc `8246FD40` (default.xex SHA-256
//! 1db39496585c521d17a2137804f42cf73ebed2b32cac166ec42dbf772f4dcf7f),
//! TU3 `0x82481E10`. Callers pass the x and y arrays of an authored
//! `PointGraphData`, or those after a `PointNegGraphData` header.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PointGraph<const N: usize> {
    pub x: [f32; N],
    pub y: [f32; N],
}

impl<const N: usize> PointGraph<N> {
    /// Native endpoint handling and slope-first binary32 interpolation.
    pub fn evaluate(&self, input: f32) -> f32 {
        assert!(N > 0);
        if input < self.x[0] {
            return self.y[0];
        }
        // The native branch enters the scan only on an ordered less-than.
        // Unordered input or a NaN final key therefore selects the last value.
        if !(input < self.x[N - 1]) {
            return self.y[N - 1];
        }
        for upper in 1..N {
            if !(input < self.x[upper]) {
                continue;
            }
            let lower = upper - 1;
            let width = self.x[upper] - self.x[lower];
            // `ble` against 0.0 (8246FDC4) is also taken for an unordered width.
            if !(width > 0.0) {
                return self.y[upper];
            }
            let slope = (self.y[upper] - self.y[lower]) / width;
            return slope.mul_add(input - self.x[lower], self.y[lower]);
        }
        self.y[0]
    }
}

#[cfg(test)]
mod tests {
    use super::PointGraph;

    #[test]
    fn endpoints_and_knots_follow_the_native_branches() {
        let graph = PointGraph {
            x: [0.0, 1.0, 1.0, 3.0],
            y: [2.0, 4.0, 6.0, 10.0],
        };
        assert_eq!(graph.evaluate(-1.0), 2.0);
        assert_eq!(graph.evaluate(0.0), 2.0);
        assert_eq!(graph.evaluate(0.5), 3.0);
        // The first strictly greater key selects the piece, so the input at a
        // repeated key continues on the piece after the repeat.
        assert_eq!(graph.evaluate(1.0), 6.0);
        assert_eq!(graph.evaluate(2.0), 8.0);
        assert_eq!(graph.evaluate(3.0), 10.0);
        assert_eq!(graph.evaluate(f32::INFINITY), 10.0);
        assert_eq!(graph.evaluate(f32::NAN), 10.0);
    }

    #[test]
    fn an_unordered_piece_width_selects_the_upper_value() {
        let graph = PointGraph {
            x: [0.0, f32::NAN, 2.0, 3.0],
            y: [1.0, 2.0, 5.0, 9.0],
        };
        // 1.5 < NaN is false, so the scan reaches x[2] with width 2 - NaN.
        assert_eq!(graph.evaluate(1.5), 5.0);
        assert_eq!(graph.evaluate(2.5), 7.0);
    }
}
