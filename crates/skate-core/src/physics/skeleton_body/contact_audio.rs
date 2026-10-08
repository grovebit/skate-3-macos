//! Body contact publication from base-disc 82BAE1D8..82BAE308 and 8274FF60.
//! This is the input signal, not the audio event selector or voice mixer.
use super::{ANIMATION_PART_COUNT, ContactRegion, SkeletonPhysicalRecord};
use crate::physics::native_arithmetic::dot3;

/// Authored physics_collision fields read by base-disc 82BAD9F0.
#[derive(Clone, Copy, Debug)]
pub struct BodyAudioSettings {
    pub strength_scale: f32, // +164
    pub force_rate: f32,     // +120
    pub normal_rate: f32,    // +124
    pub divisor: f32,        // +128
}

#[derive(Clone, Copy, Debug, Default)]
pub struct BodyAudioContact {
    pub intensity: f32,
    /// Native contact tag's low 7 bits; retained even when a region releases.
    pub material: u32,
}

#[derive(Clone, Debug, Default)]
pub struct BodyContactAudio {
    pub current: [BodyAudioContact; 8],
    pub published: [BodyAudioContact; 8],
    history: [[f32; 8]; 4],
    next: usize,
}

impl BodyContactAudio {
    pub fn update(
        &mut self,
        regions: &[ContactRegion; 8],
        physical: &SkeletonPhysicalRecord,
        part_weights: &[f32; ANIMATION_PART_COUNT],
        settings: &BodyAudioSettings,
        retained: &mut [f32; 3],
    ) {
        // The collection is unchanged across the eight native region iterations.
        let inverse = 1.0 / settings.divisor;
        let force_rate = inverse * settings.force_rate;
        let normal_rate = inverse * settings.normal_rate;
        self.current = std::array::from_fn(|i| {
            let region = regions[i];
            let intensity = region.part.map_or(0.0, |part| {
                // SkeletonState+4048 is change in pose-derived velocity;
                // +4560 is the unnormalized part weight, not COM fractions.
                let change = physical.velocity_changes[part];
                let normal_change = dot3(change, region.normal).abs();
                let force_gate = if region.force > 0.5 { 1.0 } else { 0.0 };
                let change_gate = if dot3(change, change) > f32::from_bits(0x3dcc_cccd) {
                    1.0
                } else {
                    0.0
                };
                retained[1] = (force_rate * force_gate).mul_add(region.force, retained[1]);
                retained[0] = (normal_rate * normal_change).mul_add(change_gate, retained[0]);
                let weighted = normal_change * part_weights[part];
                (weighted * settings.strength_scale)
                    .max(f32::from_bits(0x3A83_126F))
                    .clamp(0.0, 1.0)
            });
            BodyAudioContact {
                intensity,
                material: region.material_flags,
            }
        });
        for value in &mut retained[..2] {
            // Original fsel ordering also maps negative zero to positive zero.
            let low = if -*value >= 0.0 { 0.0 } else { *value };
            *value = if 1.0 - low >= 0.0 { low } else { 1.0 };
        }
        self.history[self.next] = self.current.map(|contact| contact.intensity);
        self.next = (self.next + 1) % self.history.len();
        // 8274FF60 copies the current record, then replaces only its first
        // eight strengths with four-publication maxima. Materials are not aged.
        self.published = std::array::from_fn(|i| BodyAudioContact {
            intensity: self
                .history
                .iter()
                .map(|frame| frame[i])
                .fold(0.0, f32::max),
            material: self.current[i].material,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SETTINGS: BodyAudioSettings = BodyAudioSettings {
        strength_scale: 10.0,
        force_rate: 0.5,
        normal_rate: 9.0,
        divisor: 1000.0,
    };

    #[test]
    fn retained_sums_follow_native_region_order_and_survive_contact_release() {
        let mut regions = [ContactRegion {
            part: Some(1),
            force: 1.0,
            normal: [0.0, 1.0, 0.0, 0.0],
            ..Default::default()
        }; 8];
        let mut physical = SkeletonPhysicalRecord::default();
        physical.velocity_changes[1][1] = 1.0;
        let weights = [0.025; ANIMATION_PART_COUNT];
        let mut audio = BodyContactAudio::default();
        let mut retained = [0.0, 0.0, 0.25];
        audio.update(&regions, &physical, &weights, &SETTINGS, &mut retained);
        // Base-disc replay of eight ordered contributions, including fmadds.
        assert_eq!(retained[0].to_bits(), 0x3d93_74bd);
        assert_eq!(retained[1].to_bits(), 0x3b83_126f);
        assert_eq!(retained[2], 0.25);
        let before = retained;
        for region in &mut regions {
            region.part = None;
        }
        audio.update(&regions, &physical, &weights, &SETTINGS, &mut retained);
        assert_eq!(retained, before);
        retained[..2].copy_from_slice(&[-0.0, 1.5]);
        audio.update(&regions, &physical, &weights, &SETTINGS, &mut retained);
        assert_eq!(retained[0].to_bits(), 0);
        assert_eq!(retained[1], 1.0);
    }

    #[test]
    fn normal_velocity_change_drives_intensity_not_speed_or_tangential_motion() {
        let mut regions = [ContactRegion::default(); 8];
        regions[0] = ContactRegion {
            part: Some(1),
            normal: [0.0, 1.0, 0.0, 0.0],
            material_flags: 4,
            ..Default::default()
        };
        let mut physical = SkeletonPhysicalRecord::default();
        physical.velocity_changes[1] = [100.0, -0.5, 0.0, 0.0];
        physical.velocities[1] = [0.0, 80.0, 0.0, 0.0];
        let weights = [0.025; ANIMATION_PART_COUNT];
        let mut audio = BodyContactAudio::default();
        audio.update(&regions, &physical, &weights, &SETTINGS, &mut [0.0; 3]);
        assert_eq!(audio.current[0].intensity, 0.125);
        physical.velocity_changes[1][1] = 0.0;
        audio.update(&regions, &physical, &weights, &SETTINGS, &mut [0.0; 3]);
        assert_eq!(audio.current[0].intensity.to_bits(), 0x3A83_126F);
        assert_eq!(audio.current[1].intensity, 0.0);
    }

    #[test]
    fn peaks_survive_three_more_publications_but_materials_follow_current_frame() {
        let mut regions = [ContactRegion::default(); 8];
        regions[2] = ContactRegion {
            part: Some(3),
            normal: [0.0, 1.0, 0.0, 0.0],
            material_flags: 4,
            ..Default::default()
        };
        let mut physical = SkeletonPhysicalRecord::default();
        physical.velocity_changes[3][1] = 20.0;
        let weights = [0.1; ANIMATION_PART_COUNT];
        let mut audio = BodyContactAudio::default();
        audio.update(&regions, &physical, &weights, &SETTINGS, &mut [0.0; 3]);
        assert_eq!(audio.published[2].intensity, 1.0);
        regions[2].part = None;
        regions[2].material_flags = 41;
        for _ in 0..3 {
            audio.update(&regions, &physical, &weights, &SETTINGS, &mut [0.0; 3]);
            assert_eq!(audio.current[2].intensity, 0.0);
            assert_eq!(audio.published[2].intensity, 1.0);
            assert_eq!(audio.published[2].material, 41);
        }
        audio.update(&regions, &physical, &weights, &SETTINGS, &mut [0.0; 3]);
        assert_eq!(audio.published[2].intensity, 0.0);
    }
}
