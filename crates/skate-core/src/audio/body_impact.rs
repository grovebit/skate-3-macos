//! Body-impact collision records, base-disc 824AA020..824AAA34 with helpers
//! 824AAA38, 824AAB90, 824AAD48 and 824ADA08, and the ordinary material
//! routines 82484EC8 (intensity), 82484A98 (level) and 82484D90 (hard-layer
//! scale). Speech calls 824AB928 and 824AD490 neither enqueue collisions nor
//! write this state, so they are excluded. Hall of Meat predicate 824ADA78 is
//! an explicit input. Snapshot inputs come from the caller; the 10-group voice
//! allocator, collision consumer and voice controls are separate stages.
use super::material::counterpart_class;

/// Absent-material sentinel; the consumer skips sides that carry it.
pub const ABSENT: i32 = 0x8f;
/// Head, torso, left and right arm, left and right leg. Feet use other code.
pub const REGIONS: usize = 6;
const TORSO: i32 = 0x62;

/// `aud_intensitymapping`: +00 hard, +04 maximum, +08 medium, +0C minimum.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Intensity {
    pub hard: f32,
    pub maximum: f32,
    pub medium: f32,
    pub minimum: f32,
}

/// `aud_volumemapping` integer endpoints, indexed by counterpart class.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Volume {
    /// +08/+0C, +18/+1C and +34/+38.
    pub soft: [[i32; 2]; 3],
    /// +00/+04, +10/+14 and +2C/+30.
    pub medium: [[i32; 2]; 3],
    /// +20/+24 for every class.
    pub hard: [i32; 2],
    /// +28, read by 82484D90.
    pub hard_layer_scale: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Profile {
    pub intensity: Intensity,
    pub volume: Volume,
}

/// 82484EC8 output. Silent results keep the 0.0 bounds that every body-loop
/// call site stores before classifying.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Band {
    pub category: u8,
    pub lower: f32,
    pub upper: f32,
}
impl Band {
    pub const SILENT: Self = Self {
        category: 3,
        lower: 0.,
        upper: 0.,
    };
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BodyError {
    MissingProfile(i32),
    /// Nonfinite input or an out-of-range conversion; PPC saturation and NaN
    /// conversion are outside the verified domain, not silently emulated.
    InvalidInput,
}

/// Profiles by material ID 0..0x8E, the order of table 82FD1930, and the
/// authored counterpart classes read by 82485750.
pub struct Materials {
    profiles: Vec<Option<Profile>>,
    surface_classes: [u8; 95],
}

impl Materials {
    pub fn new(profiles: Vec<Option<Profile>>, surface_classes: [u8; 95]) -> Option<Self> {
        (profiles.len() == ABSENT as usize).then_some(Self {
            profiles,
            surface_classes,
        })
    }

    fn profile(&self, material: i32) -> Result<&Profile, BodyError> {
        usize::try_from(material)
            .ok()
            .and_then(|i| self.profiles.get(i)?.as_ref())
            .ok_or(BodyError::MissingProfile(material))
    }

    /// 82484EC8 in ordinary mode. The minimum is inclusive; medium and hard
    /// are strict, tested hard first. PPC `bge`/`ble` branch on unordered
    /// compares, so an unordered strength takes the soft band.
    pub fn classify(&self, material: i32, strength: f32) -> Result<Band, BodyError> {
        if material < 0 {
            return Ok(Band::SILENT);
        }
        let t = self.profile(material)?.intensity;
        if strength < t.minimum {
            return Ok(Band::SILENT);
        }
        Ok(if strength > t.hard {
            Band {
                category: 2,
                lower: t.hard,
                upper: t.maximum,
            }
        } else if strength > t.medium {
            Band {
                category: 1,
                lower: t.medium,
                upper: t.hard,
            }
        } else {
            Band {
                category: 0,
                lower: t.minimum,
                upper: t.medium,
            }
        })
    }

    /// 82484A98 in ordinary mode. The band need not be this material's own:
    /// the cloth layer and 824AAD48 reuse the primary body bounds.
    pub fn level(
        &self,
        material: i32,
        counterpart: i32,
        band: Band,
        strength: f32,
    ) -> Result<i32, BodyError> {
        if material < 0 || band.category == 3 {
            return Ok(0);
        }
        let volume = self.profile(material)?.volume;
        // 82484B18..82484B2C: only surface-side materials consult the class.
        let class = if material < 0x61 && counterpart != ABSENT {
            usize::from(counterpart_class(counterpart, &self.surface_classes).min(2))
        } else {
            0
        };
        let [low, high] = match band.category {
            0 => volume.soft[class],
            1 => volume.medium[class],
            2 => volume.hard,
            _ => [0, 32767],
        };
        let strength = if strength < band.upper {
            strength
        } else {
            band.upper
        };
        // Negated compares keep PPC `ble`/`bge` behavior for unordered values.
        if !(band.upper > band.lower) {
            return Ok(high);
        }
        let slope = high.wrapping_sub(low) as f32 / (band.upper - band.lower);
        truncate(slope.mul_add(strength - band.lower, low as f32))
    }

    /// 82484D90; IDs outside 0..0x8E return the constant 1.0 at 82314D90.
    pub fn hard_layer_scale(&self, material: i32) -> Result<f32, BodyError> {
        if !(0..ABSENT).contains(&material) {
            return Ok(1.);
        }
        Ok(self.profile(material)?.volume.hard_layer_scale)
    }
}

/// `aud_collisions/default` values: the cooldown read in the loop and the
/// bands 824A9CD8 stores at Contacts +128..+14C.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BodySettings {
    /// `Hash_6DD85F43C1B6E6AA`, an Int32 converted to float when assigned.
    pub cooldown: i32,
    /// +128/+12C.
    pub torso_bonecrack: [f32; 2],
    /// +130/+134.
    pub head_bonecrack: [f32; 2],
    /// +138/+13C.
    pub bonesnap: [f32; 2],
    /// +140/+144.
    pub facehit_soft: [f32; 2],
    /// +148/+14C.
    pub facehit_medium: [f32; 2],
}

/// Inputs from one Contacts update. Field names follow their traced sources;
/// their gameplay meanings are not assumed.
#[derive(Clone, Copy, Debug)]
pub struct BodyFrame {
    /// Snapshot +1F0: filtered region strengths.
    pub strengths: [f32; REGIONS],
    /// Snapshot +230: region contact tags. Tag `n` selects material `n - 1`.
    pub surface_tags: [i32; REGIONS],
    /// Snapshot +DC, the publication timing ratio.
    pub publication_ratio: f32,
    /// Snapshot +2A4: PhysOut_State +3B, processed input +2468 bit 18.
    pub state_2a4: bool,
    /// Snapshot +2A5: PhysOut_Skeleton +257, the settled-wipeout byte that
    /// 82D13DA0 writes at wipeout physics +1E0.
    pub state_2a5: bool,
    /// Snapshot +251: SkeletonState +FA9, the face (part 1) specific contact.
    pub face_contact: bool,
    /// Audio manager +34B.
    pub hall_of_meat: bool,
    /// 824ADA78 for each region; read only when a Hall of Meat layer exists.
    pub hall_of_meat_layers: [bool; REGIONS],
    /// Channel-5 object (context +29CB4): float +18 when byte +10 is set.
    pub channel_5: Option<f32>,
    /// Contacts +1C: byte +48 and whether word +40 is zero.
    pub owner: (u8, bool),
}

/// The 0x30-byte record passed to 82474D30, excluding the +10 position,
/// which is snapshot +30 for every record of the update.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Record {
    /// +00 and +04.
    pub materials: [i32; 2],
    /// +08 and +0C.
    pub categories: [u8; 2],
    /// +20 and +24.
    pub levels: [i32; 2],
    /// Bytes +28, +29 and +2A.
    pub flags: [u8; 3],
}

#[derive(Clone, Debug, PartialEq)]
pub struct BodyUpdate {
    /// Records in enqueue order.
    pub records: Vec<Record>,
    /// Final values written to Contacts controller input words 7 and 8, if a
    /// controller is attached (824AA048..824AA0F8, 824AAD88..824AADA8).
    pub controller_words: [u16; 2],
}

/// Contacts +104..+118 and +1A4/+1A5, zeroed only by constructor 824A5B70.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct BodyImpacts {
    cooldowns: [f32; REGIONS],
    edge: bool,
    previous_channel_5: bool,
}

impl BodyImpacts {
    pub fn cooldowns(&self) -> [f32; REGIONS] {
        self.cooldowns
    }

    /// One call of 824AA020. State changes only if the whole update succeeds.
    pub fn update(
        &mut self,
        frame: &BodyFrame,
        materials: &Materials,
        settings: &BodySettings,
    ) -> Result<BodyUpdate, BodyError> {
        if !frame.strengths.iter().all(|v| v.is_finite())
            || !frame.publication_ratio.is_finite()
            || frame.channel_5.is_some_and(|v| !v.is_finite())
        {
            return Err(BodyError::InvalidInput);
        }
        let mut next = *self;
        let word_8 = frame
            .channel_5
            .map_or(0, |v| ((v * 32767.) as i32).clamp(0, 32767) as u16);
        let mut update = BodyUpdate {
            records: Vec::new(),
            controller_words: [0, word_8],
        };
        let flag = frame.channel_5.is_some();
        if flag && !next.previous_channel_5 {
            next.edge = true;
        }
        next.previous_channel_5 = flag;
        if !(frame.state_2a4 && frame.state_2a5) {
            for region in 0..REGIONS {
                let strength = frame.strengths[region];
                if strength > 0. && !(next.cooldowns[region] > 0.) {
                    next.region(region, strength, frame, materials, settings, &mut update)?;
                }
                if next.cooldowns[region] > 0. {
                    // No clamp at zero; a new timer is decremented at once.
                    let step = if !(frame.publication_ratio > 1.) {
                        frame.publication_ratio
                    } else {
                        1.
                    };
                    next.cooldowns[region] -= step;
                }
            }
        }
        *self = next;
        Ok(update)
    }

    fn region(
        &mut self,
        region: usize,
        strength: f32,
        frame: &BodyFrame,
        materials: &Materials,
        settings: &BodySettings,
        update: &mut BodyUpdate,
    ) -> Result<(), BodyError> {
        let [primary, secondary, tertiary, quaternary] =
            layers(region, frame.face_contact, frame.hall_of_meat);
        let tag = frame.surface_tags[region];
        let surface = if (1..=ABSENT + 1).contains(&tag) {
            tag - 1
        } else {
            ABSENT
        };
        let body = materials.classify(primary, strength)?;
        let ground = if surface < ABSENT {
            materials.classify(surface, strength)?
        } else {
            Band::SILENT
        };
        if body.category == 3 && ground.category == 3 {
            return Ok(());
        }
        self.cooldowns[region] = settings.cooldown as f32;
        let body_level = materials.level(primary, surface, body, strength)?;
        let surface_level = if surface < ABSENT {
            materials.level(surface, primary, ground, strength)?
        } else {
            0
        };
        let (owner, first) = frame.owner;
        let tail = u8::from(owner != 0 && first);
        let mut push = |materials, categories, levels, flags| {
            update.records.push(Record {
                materials,
                categories,
                levels,
                flags,
            })
        };
        push(
            [primary, surface],
            [body.category, ground.category],
            [body_level, surface_level],
            [0, owner, tail],
        );
        // 824AA58C..824AA678: a medium layer for each hard side.
        let overlay = [body.category, ground.category].map(|c| if c == 2 { 1 } else { 3 });
        if overlay != [3, 3] {
            let scaled = |material, level: i32| -> Result<i32, BodyError> {
                truncate(materials.hard_layer_scale(material)? * level as f32)
            };
            push(
                [primary, surface],
                overlay,
                [
                    scaled(primary, body_level)?,
                    scaled(surface, surface_level)?,
                ],
                [0, 0, tail],
            );
        }
        // Cloth: soft endpoints over the primary's own band.
        if secondary != ABSENT {
            let band = Band {
                category: 0,
                ..body
            };
            let level = materials.level(primary, surface, band, strength)?;
            push([secondary, ABSENT], [0, 0], [level, 0], [0, 0, tail]);
        }
        if tertiary != ABSENT {
            let band = extra_band(materials, settings, primary, tertiary, strength)?;
            let above = band.category >= 1 && strength > band.upper;
            if !above && band.category != 3 {
                let level = materials.level(tertiary, surface, band, strength)?;
                push(
                    [tertiary, ABSENT],
                    [band.category, 0],
                    [level, 0],
                    [0, 0, tail],
                );
            }
        }
        if quaternary != ABSENT && frame.hall_of_meat_layers[region] {
            let band = extra_band(materials, settings, primary, quaternary, strength)?;
            if band.category != 3 {
                let level = materials.level(quaternary, surface, band, strength)?;
                push(
                    [quaternary, ABSENT],
                    [band.category, 0],
                    [level, 0],
                    [0, 0, tail],
                );
            }
        }
        // 824AAD48: the first audible region after a channel-5 rising edge.
        if self.edge {
            update.controller_words[0] = 32767;
            if region != 1 && body.category >= 1 {
                let level = materials.level(TORSO, surface, body, strength)?;
                push([TORSO, ABSENT], [1, 1], [level, 0], [0, 0, tail]);
            }
        }
        self.edge = false;
        Ok(())
    }
}

/// 824AAA38 and 824ADA08: primary, cloth, bone/face and Hall of Meat layers.
fn layers(region: usize, face_contact: bool, hall_of_meat: bool) -> [i32; 4] {
    let (primary, secondary, tertiary) = match region {
        0 => (0x61, ABSENT, if face_contact { 0x70 } else { 0x6e }),
        1 => (0x62, 0x6d, 0x6e),
        2 | 3 => (0x64, 0x6b, 0x6f),
        _ => (0x63, 0x6c, 0x6f),
    };
    let quaternary = if hall_of_meat { primary + 5 } else { ABSENT };
    [primary, secondary, tertiary, quaternary]
}

/// 824AAB90. Bands are strict lower bounds; unlisted materials stay silent.
fn extra_band(
    materials: &Materials,
    settings: &BodySettings,
    primary: i32,
    material: i32,
    strength: f32,
) -> Result<Band, BodyError> {
    let band = |category, [lower, upper]: [f32; 2]| {
        (strength > lower).then_some(Band {
            category,
            lower,
            upper,
        })
    };
    Ok(match material {
        0x66..=0x6a => return materials.classify(material, strength),
        0x6e => match primary {
            0x61 => band(1, settings.head_bonecrack),
            0x62 => band(1, settings.torso_bonecrack),
            _ => None,
        },
        0x6f => band(1, settings.bonesnap),
        0x70 => band(1, settings.facehit_medium).or_else(|| band(0, settings.facehit_soft)),
        _ => None,
    }
    .unwrap_or(Band::SILENT))
}

/// fctiwz on values the port can represent exactly.
pub(crate) fn truncate(value: f32) -> Result<i32, BodyError> {
    if value.is_finite() && (-2147483648.0..2147483648.0).contains(&value) {
        Ok(value as i32)
    } else {
        Err(BodyError::InvalidInput)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Thresholds 0.125/0.25/0.5/1.0 keep the interpolation exact. Endpoints
    /// encode category and counterpart class so a wrong lookup is visible.
    fn profile(base: i32, scale: f32) -> Profile {
        Profile {
            intensity: Intensity {
                hard: 0.5,
                maximum: 1.,
                medium: 0.25,
                minimum: 0.125,
            },
            volume: Volume {
                soft: [0, 1, 2].map(|c| [base + c, base + c + 1000]),
                medium: [0, 1, 2].map(|c| [base + 2000 + c, base + 3000 + c]),
                hard: [base + 4000, base + 6000],
                hard_layer_scale: scale,
            },
        }
    }

    const SURFACE: i32 = 2;

    fn materials() -> Materials {
        let mut profiles = vec![None; ABSENT as usize];
        profiles[SURFACE as usize] = Some(profile(10000, 0.5));
        for (id, base) in [(0x61, 100), (0x62, 200), (0x63, 300), (0x64, 400)] {
            profiles[id] = Some(profile(base, 0.25));
        }
        for id in 0x66..=0x70 {
            profiles[id] = Some(profile(id as i32 * 100, 1.));
        }
        let mut classes = [2; 95];
        classes[SURFACE as usize] = 1;
        Materials::new(profiles, classes).unwrap()
    }

    fn settings() -> BodySettings {
        BodySettings {
            cooldown: 15,
            torso_bonecrack: [0.5, 1.],
            head_bonecrack: [0.625, 1.],
            bonesnap: [0.5, 1.],
            facehit_soft: [0.0625, 0.25],
            facehit_medium: [0.25, 0.5],
        }
    }

    fn frame() -> BodyFrame {
        BodyFrame {
            strengths: [0.; REGIONS],
            surface_tags: [0; REGIONS],
            publication_ratio: 1.,
            state_2a4: false,
            state_2a5: false,
            face_contact: false,
            hall_of_meat: false,
            hall_of_meat_layers: [false; REGIONS],
            channel_5: None,
            owner: (7, true),
        }
    }

    fn record(
        materials: [i32; 2],
        categories: [u8; 2],
        levels: [i32; 2],
        flags: [u8; 3],
    ) -> Record {
        Record {
            materials,
            categories,
            levels,
            flags,
        }
    }

    #[test]
    fn classifier_and_levels_keep_native_bounds_classes_and_caps() {
        let m = materials();
        let band = |category, lower, upper| Band {
            category,
            lower,
            upper,
        };
        assert_eq!(m.classify(0x62, 0.124), Ok(Band::SILENT));
        assert_eq!(m.classify(0x62, f32::NAN), Ok(band(0, 0.125, 0.25)));
        assert_eq!(m.classify(-1, 1.), Ok(Band::SILENT));
        assert_eq!(m.classify(0x62, 0.125), Ok(band(0, 0.125, 0.25)));
        assert_eq!(m.classify(0x62, 0.25), Ok(band(0, 0.125, 0.25)));
        assert_eq!(m.classify(0x62, 0.5), Ok(band(1, 0.25, 0.5)));
        assert_eq!(m.classify(0x62, 0.75), Ok(band(2, 0.5, 1.)));
        assert_eq!(m.classify(0x65, 1.), Err(BodyError::MissingProfile(0x65)));
        // Body materials ignore the counterpart; surfaces use 82485750.
        assert_eq!(
            m.level(0x62, SURFACE, band(0, 0.125, 0.25), 0.1875),
            Ok(700)
        );
        assert_eq!(
            m.level(SURFACE, 0x62, band(0, 0.125, 0.25), 0.1875),
            Ok(10500)
        );
        assert_eq!(m.level(SURFACE, 0x61, band(1, 0.25, 0.5), 0.375), Ok(12501));
        assert_eq!(
            m.level(SURFACE, 0x6e, band(0, 0.125, 0.25), 0.25),
            Ok(11002)
        );
        assert_eq!(m.level(SURFACE, ABSENT, band(2, 0.5, 1.), 2.), Ok(16000));
        // Degenerate or reversed bounds return the upper endpoint.
        assert_eq!(m.level(0x62, ABSENT, band(1, 0.5, 0.5), 0.), Ok(3200));
        assert_eq!(m.level(0x62, ABSENT, Band::SILENT, 1.), Ok(0));
        assert_eq!(m.hard_layer_scale(ABSENT), Ok(1.));
        assert_eq!(m.hard_layer_scale(0x62), Ok(0.25));
    }

    #[test]
    fn hard_torso_hit_enqueues_pair_overlay_cloth_and_bone_layers_in_order() {
        let m = materials();
        let mut body = BodyImpacts::default();
        let mut input = frame();
        input.strengths[1] = 0.75;
        input.surface_tags[1] = SURFACE + 1;
        input.publication_ratio = 0.5;
        let update = body.update(&input, &m, &settings()).unwrap();
        assert_eq!(
            update.records,
            [
                record([0x62, SURFACE], [2, 2], [5200, 15000], [0, 7, 1]),
                record([0x62, SURFACE], [1, 1], [1300, 7500], [0, 0, 1]),
                // Cotton uses torso soft endpoints over the hard band.
                record([0x6d, ABSENT], [0, 0], [700, 0], [0, 0, 1]),
                record([0x6e, ABSENT], [1, 0], [13500, 0], [0, 0, 1]),
            ]
        );
        assert_eq!(update.controller_words, [0, 0]);
        // Assigned and decremented by min(ratio, 1) in the same update.
        assert_eq!(body.cooldowns(), [0., 14.5, 0., 0., 0., 0.]);
    }

    #[test]
    fn cooldown_gate_and_publication_ratio_follow_the_body_loop() {
        let m = materials();
        let mut body = BodyImpacts::default();
        let mut input = frame();
        input.strengths[2] = 0.75;
        input.publication_ratio = 4.;
        let first = body.update(&input, &m, &settings()).unwrap();
        assert_eq!(
            first.records[0],
            record([0x64, ABSENT], [2, 3], [5400, 0], [0, 7, 1])
        );
        assert_eq!(body.cooldowns()[2], 14.);
        // Positive timers block new records and keep counting down.
        assert!(
            body.update(&input, &m, &settings())
                .unwrap()
                .records
                .is_empty()
        );
        assert_eq!(body.cooldowns()[2], 13.);
        // Both snapshot flags skip the loop, including the decrement.
        input.state_2a4 = true;
        input.state_2a5 = true;
        assert!(
            body.update(&input, &m, &settings())
                .unwrap()
                .records
                .is_empty()
        );
        assert_eq!(body.cooldowns()[2], 13.);
        input.state_2a5 = false;
        input.publication_ratio = 6.5;
        for _ in 0..13 {
            body.update(&input, &m, &settings()).unwrap();
        }
        assert_eq!(body.cooldowns()[2], 0.);
        assert_eq!(
            body.update(&input, &m, &settings()).unwrap().records.len(),
            4
        );
        // A silent pair neither enqueues nor rearms the timer.
        let mut quiet = BodyImpacts::default();
        input.strengths = [0.0625; REGIONS];
        assert!(
            quiet
                .update(&input, &m, &settings())
                .unwrap()
                .records
                .is_empty()
        );
        assert_eq!(quiet.cooldowns(), [0.; REGIONS]);
    }

    #[test]
    fn channel_5_edge_publishes_controls_and_one_torso_layer() {
        let m = materials();
        let mut body = BodyImpacts::default();
        let mut input = frame();
        input.channel_5 = Some(0.5);
        // The rising edge waits for an audible region.
        let update = body.update(&input, &m, &settings()).unwrap();
        assert_eq!(update.controller_words, [0, 16383]);
        input.channel_5 = Some(2.);
        input.strengths[0] = 0.375;
        input.strengths[4] = 0.375;
        let update = body.update(&input, &m, &settings()).unwrap();
        assert_eq!(update.controller_words, [32767, 32767]);
        let torso: Vec<_> = update
            .records
            .iter()
            .filter(|r| r.materials == [0x62, ABSENT])
            .collect();
        assert_eq!(
            torso,
            [&record([0x62, ABSENT], [1, 1], [2700, 0], [0, 0, 1])]
        );
        input.strengths[0] = 0.;
        input.strengths[4] = 0.;
        input.strengths[3] = 0.375;
        let update = body.update(&input, &m, &settings()).unwrap();
        assert_eq!(update.controller_words, [0, 32767]);
        assert!(update.records.iter().all(|r| r.materials[0] != 0x62));
        input.channel_5 = Some(-1.);
        assert_eq!(
            body.update(&input, &m, &settings())
                .unwrap()
                .controller_words,
            [0, 0]
        );
    }

    #[test]
    fn face_contact_bands_and_hall_of_meat_layers_are_explicitly_gated() {
        let m = materials();
        let mut input = frame();
        input.strengths[0] = 0.375;
        input.face_contact = true;
        let records = BodyImpacts::default()
            .update(&input, &m, &settings())
            .unwrap()
            .records;
        assert_eq!(
            records.last(),
            Some(&record([0x70, ABSENT], [1, 0], [13700, 0], [0, 0, 1]))
        );
        // Medium facehit above its upper band adds nothing; no soft fallback.
        input.strengths[0] = 0.75;
        let records = BodyImpacts::default()
            .update(&input, &m, &settings())
            .unwrap()
            .records;
        assert!(records.iter().all(|r| r.materials[0] != 0x70));
        input.face_contact = false;
        let records = BodyImpacts::default()
            .update(&input, &m, &settings())
            .unwrap()
            .records;
        assert_eq!(
            records.last(),
            Some(&record([0x6e, ABSENT], [1, 0], [13333, 0], [0, 0, 1]))
        );
        input.hall_of_meat = true;
        let records = BodyImpacts::default()
            .update(&input, &m, &settings())
            .unwrap()
            .records;
        assert!(records.iter().all(|r| r.materials[0] != 0x66));
        input.hall_of_meat_layers[0] = true;
        let records = BodyImpacts::default()
            .update(&input, &m, &settings())
            .unwrap()
            .records;
        assert_eq!(
            records.last(),
            Some(&record([0x66, ABSENT], [2, 0], [15200, 0], [0, 0, 1]))
        );
    }

    #[test]
    fn invalid_frames_and_missing_profiles_leave_state_unchanged() {
        let m = materials();
        let mut body = BodyImpacts::default();
        let mut input = frame();
        input.channel_5 = Some(1.);
        input.strengths[1] = 0.75;
        input.surface_tags[1] = 0x66 + 1 - 0x60; // Material 6 has no profile.
        let before = body;
        assert_eq!(
            body.update(&input, &m, &settings()),
            Err(BodyError::MissingProfile(6))
        );
        assert_eq!(body, before);
        input.surface_tags[1] = 0;
        input.publication_ratio = f32::INFINITY;
        assert_eq!(
            body.update(&input, &m, &settings()),
            Err(BodyError::InvalidInput)
        );
        assert_eq!(body, before);
    }
}
