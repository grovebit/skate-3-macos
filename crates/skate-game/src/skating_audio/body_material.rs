//! Ordinary primary-body material evaluation, base-disc 82484EC8/82484A98.
//! Includes the primary hard-to-medium layer at 824AA590..824AA678.
//! This does not schedule events, select other body layers, or convert voice gain.
use serde::Deserialize;
use std::collections::HashMap;

#[derive(Deserialize)]
pub(super) struct Tables<T> {
    materials: HashMap<String, Option<String>>,
    tables: HashMap<String, T>,
}
impl<T> Tables<T> {
    fn material(&self, name: &str) -> Option<&T> {
        self.tables.get(self.materials.get(name)?.as_ref()?)
    }
}

#[derive(Clone, Copy, Deserialize)]
pub(super) struct Intensity {
    minimum: f32,
    medium: f32,
    hard: f32,
    maximum: f32,
}
#[derive(Clone, Copy, Deserialize)]
pub(super) struct Volume {
    // Body materials >= 0x61 always use counterpart class zero. Other
    // counterpart classes remain in the export for the surface-side port.
    soft_0_min: i32,
    soft_0_max: i32,
    medium_0_min: i32,
    medium_0_max: i32,
    hard_min: i32,
    hard_max: i32,
    // Older exports lack this value; do not invent an audible layer for them.
    #[serde(default)]
    hard_layer_scale: Option<f32>,
}
#[derive(Clone, Copy)]
struct Profile {
    intensity: Intensity,
    volume: Volume,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct BodyLayers {
    pub primary: Option<(u8, i32)>,
    /// None means either no hard classification or an unavailable authored scale.
    pub medium_overlay: Option<(u8, i32)>,
}

pub(super) struct BodyMaterials([Profile; 6]);
impl BodyMaterials {
    pub fn load(intensities: Tables<Intensity>, volumes: Tables<Volume>) -> Option<Self> {
        let mut profiles = Vec::with_capacity(6);
        // 824AAA38: head, torso, two arms, two legs. Foot consumers differ.
        for name in ["head", "torso", "arm", "arm", "leg", "leg"] {
            let intensity = *intensities.material(name)?;
            let volume = *volumes.material(name)?;
            if ![
                intensity.minimum,
                intensity.medium,
                intensity.hard,
                intensity.maximum,
            ]
            .into_iter()
            .all(|v| v.is_finite() && v >= 0.)
                || ![
                    volume.soft_0_min,
                    volume.soft_0_max,
                    volume.medium_0_min,
                    volume.medium_0_max,
                    volume.hard_min,
                    volume.hard_max,
                ]
                .into_iter()
                .all(|v| (0..=32767).contains(&v))
                || volume
                    .hard_layer_scale
                    .is_some_and(|v| !v.is_finite() || v < 0.)
            {
                return None;
            }
            profiles.push(Profile { intensity, volume });
        }
        Some(Self(profiles.try_into().ok()?))
    }

    /// Raw primary material decisions, before cooldowns and other body-loop gates.
    pub fn evaluate(&self, strengths: [f32; 6]) -> [BodyLayers; 6] {
        std::array::from_fn(|i| self.0[i].layers(strengths[i]))
    }
}

impl Profile {
    fn layers(self, strength: f32) -> BodyLayers {
        let primary = self.evaluate(strength);
        // The extra category-1 layer uses the already interpolated hard level,
        // not a second evaluation through the medium volume endpoints.
        let medium_overlay = primary.and_then(|(category, level)| {
            if category != 2 {
                return None;
            }
            let scale = self.volume.hard_layer_scale?;
            Some((1, (level as f32 * scale) as i32))
        });
        BodyLayers {
            primary,
            medium_overlay,
        }
    }

    fn evaluate(self, strength: f32) -> Option<(u8, i32)> {
        let t = self.intensity;
        if !strength.is_finite() || strength < t.minimum {
            return None;
        }
        // Preserve branch order, even for unordered authored bounds.
        let (category, lower, upper, low_level, high_level) = if strength > t.hard {
            (
                2,
                t.hard,
                t.maximum,
                self.volume.hard_min,
                self.volume.hard_max,
            )
        } else if strength > t.medium {
            (
                1,
                t.medium,
                t.hard,
                self.volume.medium_0_min,
                self.volume.medium_0_max,
            )
        } else {
            (
                0,
                t.minimum,
                t.medium,
                self.volume.soft_0_min,
                self.volume.soft_0_max,
            )
        };
        let level = if upper <= lower {
            high_level
        } else {
            let slope = (high_level - low_level) as f32 / (upper - lower);
            slope.mul_add(strength.min(upper) - lower, low_level as f32) as i32
        };
        Some((category, level))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn profile() -> Profile {
        Profile {
            intensity: Intensity {
                minimum: 0.125,
                medium: 0.25,
                hard: 0.5,
                maximum: 1.,
            },
            volume: Volume {
                soft_0_min: 100,
                soft_0_max: 200,
                medium_0_min: 300,
                medium_0_max: 500,
                hard_min: 600,
                hard_max: 900,
                hard_layer_scale: Some(0.625),
            },
        }
    }

    #[test]
    fn classifier_uses_strict_medium_and_hard_boundaries() {
        let p = profile();
        assert_eq!(p.evaluate(0.124), None);
        assert_eq!(p.evaluate(0.125), Some((0, 100)));
        assert_eq!(p.evaluate(0.25), Some((0, 200)));
        assert_eq!(
            p.evaluate(f32::from_bits(0.25f32.to_bits() + 1)),
            Some((1, 300))
        );
        assert_eq!(p.evaluate(0.5), Some((1, 500)));
        assert_eq!(
            p.evaluate(f32::from_bits(0.5f32.to_bits() + 1)),
            Some((2, 600))
        );
    }

    #[test]
    fn levels_interpolate_truncate_and_cap_at_upper_bound() {
        let p = profile();
        assert_eq!(p.evaluate(0.375), Some((1, 400)));
        assert_eq!(p.evaluate(0.751), Some((2, 750)));
        assert_eq!(p.evaluate(2.), Some((2, 900)));
        let mut descending = p;
        descending.volume.hard_min = 900;
        descending.volume.hard_max = 600;
        assert_eq!(descending.evaluate(0.75), Some((2, 750)));
    }

    #[test]
    fn unordered_and_degenerate_bounds_follow_original_branch_order() {
        let mut p = profile();
        p.intensity.medium = 0.75;
        assert_eq!(p.evaluate(0.625), Some((2, 675)));
        p.intensity.maximum = p.intensity.hard;
        assert_eq!(p.evaluate(0.625), Some((2, 900)));
        p.intensity.maximum = 0.25;
        assert_eq!(p.evaluate(0.625), Some((2, 900)));
        assert_eq!(p.evaluate(f32::NAN), None);
    }

    #[test]
    fn older_volume_exports_keep_primary_evaluation_without_overlay() {
        let volume: Volume = serde_json::from_str(
            r#"{
            "soft_0_min":100,"soft_0_max":200,"medium_0_min":300,
            "medium_0_max":500,"hard_min":600,"hard_max":900
        }"#,
        )
        .unwrap();
        let p = Profile {
            volume,
            ..profile()
        };
        assert_eq!(
            p.layers(0.75),
            BodyLayers {
                primary: Some((2, 750)),
                medium_overlay: None,
            }
        );
    }

    #[test]
    fn hard_layer_uses_scaled_primary_level_and_preserves_zero() {
        let mut p = profile();
        assert_eq!(
            p.layers(0.75),
            BodyLayers {
                primary: Some((2, 750)),
                medium_overlay: Some((1, 468)),
            }
        );
        assert_eq!(p.layers(0.5).medium_overlay, None);
        assert_eq!(p.layers(0.124).medium_overlay, None);
        p.volume.hard_layer_scale = Some(0.);
        assert_eq!(p.layers(0.75).medium_overlay, Some((1, 0)));
        p.volume.hard_layer_scale = Some(1.5);
        assert_eq!(p.layers(0.75).medium_overlay, Some((1, 1125)));
        p.volume.hard_layer_scale = None;
        assert_eq!(p.layers(0.75).primary, Some((2, 750)));
        assert_eq!(p.layers(0.75).medium_overlay, None);
    }

    #[test]
    fn loader_resolves_references_and_rejects_missing_null_or_invalid_inputs() {
        let p = profile();
        let banks = || {
            let names = ["head", "torso", "arm", "leg"];
            let references = names.map(|name| (name.into(), Some("mapped".into())));
            (
                Tables {
                    materials: HashMap::from(references.clone()),
                    tables: HashMap::from([("mapped".into(), p.intensity)]),
                },
                Tables {
                    materials: HashMap::from(references),
                    tables: HashMap::from([("mapped".into(), p.volume)]),
                },
            )
        };
        let (a, b) = banks();
        let loaded = BodyMaterials::load(a, b).unwrap();
        assert_eq!(
            loaded.evaluate([0.375; 6]),
            [BodyLayers {
                primary: Some((1, 400)),
                medium_overlay: None
            }; 6]
        );
        let (a, mut b) = banks();
        b.materials.insert("head".into(), None);
        assert!(BodyMaterials::load(a, b).is_none());
        let (mut a, b) = banks();
        a.tables.clear();
        assert!(BodyMaterials::load(a, b).is_none());
        let (mut a, b) = banks();
        a.tables.get_mut("mapped").unwrap().minimum = f32::INFINITY;
        assert!(BodyMaterials::load(a, b).is_none());
        let (a, mut b) = banks();
        b.tables.get_mut("mapped").unwrap().hard_max = 32768;
        assert!(BodyMaterials::load(a, b).is_none());
    }
}
