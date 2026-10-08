//! Original collision bank (`collisions.json`) for records produced by
//! the core body/deck loops. Groups follow 824DF400 and each side uses
//! the ordinary 82484410/82484638 resolution. Voice volume reads each group's
//! live MixMap controls (`mixmap/`, `skate_core::audio::collision_mix`);
//! pitch controllers share that cone and use the native voice consumer.
use bevy::prelude::*;
use serde::Deserialize;
use skate_core::audio::{
    body_impact::{ABSENT, BodySettings, Intensity, Materials, Profile, Record, Volume},
    collision_group,
    curve::CurveTable,
    material::counterpart_class,
    scalar::ScalarTables,
    voice,
};
use std::{collections::HashMap, path::Path};

/// Every packed volume slot at its maximum level. Live levels never exceed
/// it, so a gain whose arithmetic holds here holds for every live word.
pub(super) const FULL_SCALE: [u32; 16] = [0x7fff_7fff; 16];
pub(super) const GROUPS: usize = 10;

#[derive(Deserialize)]
struct Manifest {
    version: u32,
    banks: Vec<String>,
    materials: Vec<Option<MaterialEntry>>,
    settings: SettingsEntry,
    surface_classes: Vec<u8>,
    events: HashMap<String, HashMap<String, EventEntry>>,
}
#[derive(Deserialize)]
struct MaterialEntry {
    name: String,
    bank: i32,
    events: Matrix,
    authored_base_level: i32,
    pitch: i32,
    material_class: i32,
    intensity: IntensityEntry,
    volume: VolumeEntry,
}
#[derive(Clone, Copy, Deserialize)]
struct Matrix {
    soft: [u32; 3],
    medium: [u32; 3],
    hard: u32,
}
#[derive(Deserialize)]
struct IntensityEntry {
    minimum: f32,
    medium: f32,
    hard: f32,
    maximum: f32,
}
#[derive(Deserialize)]
struct VolumeEntry {
    soft_0_min: i32,
    soft_0_max: i32,
    soft_1_min: i32,
    soft_1_max: i32,
    soft_2_min: i32,
    soft_2_max: i32,
    medium_0_min: i32,
    medium_0_max: i32,
    medium_1_min: i32,
    medium_1_max: i32,
    medium_2_min: i32,
    medium_2_max: i32,
    hard_min: i32,
    hard_max: i32,
    hard_layer_scale: f32,
}
#[derive(Deserialize)]
struct SettingsEntry {
    cooldown: i32,
    /// Older owned exports keep deck playback on the provisional path.
    #[serde(default)]
    board_cooldown: Option<i32>,
    #[serde(default)]
    grind_bands: Option<[f32; 2]>,
    bands: [f32; 10],
}
#[derive(Deserialize)]
struct EventEntry {
    clips: Vec<String>,
    #[serde(default)]
    gains: Vec<f32>,
}

struct Controls {
    name: String,
    bank: u8,
    events: Matrix,
    authored_base_level: i32,
    pitch: i32,
    class: i32,
}
struct EventClips {
    clips: Vec<Handle<AudioSource>>,
    gains: Vec<f32>,
}

#[derive(Deserialize)]
struct MixManifest {
    version: u32,
    files: MixFiles,
    settings: MixSettings,
}
#[derive(Deserialize)]
struct MixFiles {
    program: String,
    volume: String,
    log: String,
    curve: String,
    pitch_semitones: String,
    pitch_cents: String,
}
/// Authored aud_general floats exported beside the program.
#[derive(Clone, Copy, Deserialize)]
pub(super) struct MixSettings {
    /// camera Hash_A6F853B935E46E5F, 3DColPos controller +70.
    pub listener_offset: f32,
    /// treatments Hash_A12258EB71B6937A, Master speed scale.
    pub treatment_scale: f32,
    /// hom Hash_4BD3E0CCEA90BE26, manager +34B threshold.
    pub hom_threshold: f32,
}
/// Owned MixMapSK8.mxb and the executable tables at 82FBB430/82FBBD98/82FBC598.
pub(super) struct MixData {
    pub program: Vec<u8>,
    pub curves: CurveTable,
    pub tables: ScalarTables,
    pub pitch: skate_core::audio::pitch::PitchTables,
    pub settings: MixSettings,
}
impl MixData {
    pub(super) fn load(root: &Path) -> Option<Self> {
        let root = root.join("mixmap");
        let read = |name: &str| skate_mods::read_bounded(&root, name, 256 * 1024).ok();
        let manifest: MixManifest = serde_json::from_slice(&read("mixmap.json")?).ok()?;
        let settings = manifest.settings;
        if manifest.version != 1
            || ![
                settings.listener_offset,
                settings.treatment_scale,
                settings.hom_threshold,
            ]
            .iter()
            .all(|v| v.is_finite())
        {
            return None;
        }
        Some(Self {
            program: read(&manifest.files.program)?,
            curves: CurveTable::from_be_bytes(&read(&manifest.files.curve)?)?,
            tables: ScalarTables::from_be_bytes(
                &read(&manifest.files.log)?,
                &read(&manifest.files.volume)?,
            )?,
            pitch: skate_core::audio::pitch::PitchTables::from_be_bytes(
                &read(&manifest.files.pitch_semitones)?,
                &read(&manifest.files.pitch_cents)?,
            )?,
            settings,
        })
    }
}

pub(super) struct CollisionBank {
    pub materials: Materials,
    pub settings: BodySettings,
    pub board_cooldown: Option<i32>,
    pub grind_bands: Option<[f32; 2]>,
    controls: Vec<Option<Controls>>,
    classes: [u8; 95],
    events: HashMap<(u8, u32), EventClips>,
}

pub(super) struct SideVoice<'a> {
    pub clip: &'a Handle<AudioSource>,
    pub material: &'a str,
    pub bank: u8,
    pub event: u32,
    pub gain: VoiceGain,
}

/// Inputs of the consumer gain 824C0334..824C03DC for one playing side,
/// recomputed whenever the group's controller words change.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct VoiceGain {
    /// Rendered clip trim (host adaptation).
    pub trim: f32,
    /// Queued record level +20/+24.
    pub level: i32,
    pub base_level: i32,
    /// Material-class volume slot (824BFF88).
    pub slot: usize,
    pub authored_pitch: i32,
    pub pitch_slot: usize,
}
impl VoiceGain {
    /// Trim times the original gain read from the group's packed words.
    pub fn volume(&self, words: &[u32]) -> f32 {
        voice::collision_gain(self.level, self.base_level, Some(words), self.slot)
            .map_or(0., |gain| self.trim * gain.gain)
    }
}

impl CollisionBank {
    /// The bank and the mix data its voices require.
    pub fn load(root: &Path, assets: &mut Assets<AudioSource>) -> Option<(Self, MixData)> {
        let root = root.join("material-impacts");
        let bytes = skate_mods::read_bounded(&root, "collisions.json", 2 * 1024 * 1024).ok()?;
        let Ok(manifest) = serde_json::from_slice::<Manifest>(&bytes) else {
            warn!("Skating audio: invalid collisions.json; body impacts stay provisional");
            return None;
        };
        // Checked before any clip is decoded: the voices cannot play without it.
        let Some(mix) = MixData::load(&root) else {
            warn!(
                "Skating audio: missing or invalid mixmap/ (python3 -m tools.audio.prepare_collision_mix); body impacts stay provisional"
            );
            return None;
        };
        let bank = Self::build(
            manifest,
            |relative| {
                let bytes = skate_mods::read_bounded(&root, relative, 2 * 1024 * 1024).ok()?;
                let (pcm, info) = skate_mods::audio::canonical_pcm_wav(&bytes).ok()?;
                // Only the original mono-to-stereo Pn21 branch is ported.
                (info.channels == 1).then_some(pcm)
            },
            assets,
        );
        let Some(bank) = bank else {
            warn!("Skating audio: unsupported collisions.json; body impacts stay provisional");
            return None;
        };
        Some((bank, mix))
    }

    fn build(
        manifest: Manifest,
        mut read: impl FnMut(&str) -> Option<Vec<u8>>,
        assets: &mut Assets<AudioSource>,
    ) -> Option<Self> {
        if manifest.version != 1
            || manifest.banks.len() > 255
            || manifest.materials.len() != ABSENT as usize
            || manifest.surface_classes.len() != 95
            || manifest.surface_classes.iter().any(|c| *c > 2)
        {
            return None;
        }
        let mut profiles = Vec::with_capacity(manifest.materials.len());
        let mut controls = Vec::with_capacity(manifest.materials.len());
        for entry in manifest.materials {
            let Some(entry) = entry else {
                profiles.push(None);
                controls.push(None);
                continue;
            };
            let bank = u8::try_from(entry.bank).ok()?;
            if usize::from(bank) >= manifest.banks.len() || entry.name.is_empty() {
                return None;
            }
            profiles.push(Some(profile(&entry.intensity, &entry.volume)?));
            controls.push(Some(Controls {
                name: entry.name,
                bank,
                events: entry.events,
                authored_base_level: entry.authored_base_level,
                pitch: entry.pitch,
                class: entry.material_class,
            }));
        }
        // Contacts +128..+14C order: torso and head bonecrack, bonesnap,
        // then soft and medium facehit, each as lower/upper.
        let bands = manifest.settings.bands;
        if !bands.iter().all(|v| v.is_finite()) {
            return None;
        }
        let pair = |i: usize| [bands[i], bands[i + 1]];
        let settings = BodySettings {
            cooldown: manifest.settings.cooldown,
            torso_bonecrack: pair(0),
            head_bonecrack: pair(2),
            bonesnap: pair(4),
            facehit_soft: pair(6),
            facehit_medium: pair(8),
        };
        let mut cache = HashMap::<String, Option<Handle<AudioSource>>>::new();
        let mut budget = 96 * 1024 * 1024;
        let mut events = HashMap::new();
        for (index, name) in manifest.banks.iter().enumerate() {
            let Some(bank) = manifest.events.get(name) else {
                continue;
            };
            for (event, entry) in bank {
                let event: u32 = event.parse().ok()?;
                if entry.clips.is_empty()
                    || entry.clips.len() > 4
                    || (!entry.gains.is_empty() && entry.gains.len() != entry.clips.len())
                    || entry
                        .gains
                        .iter()
                        .any(|g| !g.is_finite() || *g <= 0. || *g > 1.)
                {
                    return None;
                }
                let mut clips = Vec::new();
                let mut gains = Vec::new();
                for (i, relative) in entry.clips.iter().enumerate() {
                    let clip = cache.entry(relative.clone()).or_insert_with(|| {
                        let pcm = read(relative)?;
                        if relative.len() > 128 || pcm.len() > budget {
                            return None;
                        }
                        budget -= pcm.len();
                        Some(assets.add(AudioSource { bytes: pcm.into() }))
                    });
                    if let Some(clip) = clip {
                        clips.push(clip.clone());
                        gains.push(entry.gains.get(i).copied().unwrap_or(1.));
                    }
                }
                if !clips.is_empty() {
                    events.insert((index as u8, event), EventClips { clips, gains });
                }
            }
        }
        info!(
            "Skating audio: original collision bank loaded with {} events",
            events.len()
        );
        Some(Self {
            materials: Materials::new(profiles, manifest.surface_classes.clone().try_into().ok()?)?,
            settings,
            board_cooldown: manifest.settings.board_cooldown,
            grind_bands: manifest.settings.grind_bands,
            controls,
            classes: manifest.surface_classes.try_into().ok()?,
            events,
        })
    }

    /// Ordinary 82484410 resolution for one side of a queued record. Sides with
    /// the absent material or category 3 are skipped (824BFE70..824BFE84).
    /// Record +28 special pitch, bank-8 and Hall of Meat set overrides are not
    /// applied; body records carry +28 = 0.
    pub fn side(&self, record: &Record, side: usize, variant: usize) -> Option<SideVoice<'_>> {
        let material = record.materials[side];
        let category = record.categories[side];
        if material == ABSENT || category == 3 {
            return None;
        }
        let controls = self
            .controls
            .get(usize::try_from(material).ok()?)?
            .as_ref()?;
        let counterpart = record.materials[1 - side];
        let class = if counterpart == ABSENT {
            0
        } else {
            usize::from(counterpart_class(counterpart, &self.classes).min(2))
        };
        let event = match category {
            0 => controls.events.soft[class],
            1 => controls.events.medium[class],
            2 => controls.events.hard,
            _ => return None,
        };
        let clips = self.events.get(&(controls.bank, event))?;
        let slot = voice::volume_slot(controls.class);
        // Validate the arithmetic domain once; live words change the level.
        voice::collision_gain(
            record.levels[side],
            controls.authored_base_level,
            Some(&FULL_SCALE),
            slot,
        )
        .ok()?;
        // No gain gate: zero-gain voices still hold their group.
        let index = variant % clips.clips.len();
        Some(SideVoice {
            clip: &clips.clips[index],
            material: &controls.name,
            bank: controls.bank,
            event,
            gain: VoiceGain {
                trim: clips.gains[index],
                level: record.levels[side],
                base_level: controls.authored_base_level,
                slot,
                authored_pitch: controls.pitch,
                pitch_slot: skate_core::audio::pitch::pitch_slot(material, controls.class),
            },
        })
    }
}

fn profile(intensity: &IntensityEntry, volume: &VolumeEntry) -> Option<Profile> {
    let v = volume;
    let levels = [
        v.soft_0_min,
        v.soft_0_max,
        v.soft_1_min,
        v.soft_1_max,
        v.soft_2_min,
        v.soft_2_max,
        v.medium_0_min,
        v.medium_0_max,
        v.medium_1_min,
        v.medium_1_max,
        v.medium_2_min,
        v.medium_2_max,
        v.hard_min,
        v.hard_max,
    ];
    let bounds = [
        intensity.minimum,
        intensity.medium,
        intensity.hard,
        intensity.maximum,
    ];
    if !levels.iter().all(|l| (0..=32767).contains(l))
        || !bounds.iter().all(|b| b.is_finite() && *b >= 0.)
        || !(v.hard_layer_scale.is_finite() && v.hard_layer_scale >= 0.)
    {
        return None;
    }
    Some(Profile {
        intensity: Intensity {
            hard: intensity.hard,
            maximum: intensity.maximum,
            medium: intensity.medium,
            minimum: intensity.minimum,
        },
        volume: Volume {
            soft: [
                [v.soft_0_min, v.soft_0_max],
                [v.soft_1_min, v.soft_1_max],
                [v.soft_2_min, v.soft_2_max],
            ],
            medium: [
                [v.medium_0_min, v.medium_0_max],
                [v.medium_1_min, v.medium_1_max],
                [v.medium_2_min, v.medium_2_max],
            ],
            hard: [v.hard_min, v.hard_max],
            hard_layer_scale: v.hard_layer_scale,
        },
    })
}

/// The manager's ten collision groups, in creation order (828B7318). Group
/// i owns collision-family MixMap instance i.
#[derive(Default)]
pub(super) struct Groups([Group; GROUPS]);
#[derive(Clone, Default)]
struct Group {
    active: bool,
    stamp: u32,
    /// Owner +44 record and its +10 position, stored at enqueue (82474D30).
    record: Option<(Record, [f32; 4])>,
    /// A MixMap phase has written controls for this record. The original
    /// output pass (824C01B8) follows the evaluator in the same phase.
    ready: bool,
    /// Initial playback 824BFE08 has run for this record.
    started: bool,
    voices: Vec<(Entity, VoiceGain)>,
}

impl Groups {
    /// 824DF400 and 824E6728: the first inactive group, else the active group
    /// with the smallest stamp below u32::MAX (ties keep the earlier group),
    /// which is reset before reuse. `None` drops the record, as the native
    /// does. Returns the group, whether it was reset for reuse, and the
    /// voices that reset stops.
    pub fn allocate(
        &mut self,
        stamp: u32,
        record: Record,
        position: [f32; 4],
    ) -> Option<(usize, bool, Vec<Entity>)> {
        let snapshots: [_; GROUPS] = std::array::from_fn(|index| collision_group::Group {
            active: u8::from(self.0[index].active),
            timestamp: self.0[index].stamp,
        });
        let selection = collision_group::select(&snapshots)?;
        let index = selection.index;
        let group = &mut self.0[index];
        let stolen = group.voices.drain(..).map(|(voice, _)| voice).collect();
        *group = Group {
            active: true,
            stamp,
            record: Some((record, position)),
            ready: false,
            started: false,
            voices: Vec::new(),
        };
        Some((index, selection.reset, stolen))
    }

    /// After a MixMap phase, every active group has fresh controls.
    pub fn evaluated(&mut self) {
        for group in &mut self.0 {
            group.ready |= group.active;
        }
    }

    /// Active groups' record positions, for 3DColPos publication.
    pub fn positions(&self) -> impl Iterator<Item = (usize, [f32; 4])> + '_ {
        self.0
            .iter()
            .enumerate()
            .filter_map(|(i, g)| g.active.then(|| g.record.map(|(_, p)| (i, p))).flatten())
    }

    /// Active, evaluated groups whose initial playback has not run.
    pub fn unstarted(&self) -> Vec<(usize, Record)> {
        self.0
            .iter()
            .enumerate()
            .filter(|(_, g)| g.active && g.ready && !g.started)
            .filter_map(|(i, g)| g.record.map(|(record, _)| (i, record)))
            .collect()
    }

    pub fn start(&mut self, group: usize) {
        self.0[group].started = true;
    }

    pub fn attach(&mut self, group: usize, voice: Entity, gain: VoiceGain) {
        self.0[group].voices.push((voice, gain));
    }

    /// Playing voices with their gain inputs, by group.
    pub fn voices(&self) -> impl Iterator<Item = (usize, Entity, VoiceGain)> + '_ {
        self.0
            .iter()
            .enumerate()
            .flat_map(|(i, g)| g.voices.iter().map(move |(e, v)| (i, *e, *v)))
    }

    /// Completion resets the group once both channels clear (824C0440..0468).
    /// Host entities represent playback channels. Returns the groups reset.
    pub fn retain(&mut self, mut playing: impl FnMut(Entity) -> bool) -> Vec<usize> {
        let mut reset = Vec::new();
        for (index, group) in self.0.iter_mut().enumerate() {
            group.voices.retain(|(voice, _)| playing(*voice));
            if group.active && group.started && group.voices.is_empty() {
                *group = Group::default();
                reset.push(index);
            }
        }
        reset
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn groups_fill_in_order_then_steal_the_oldest_with_earlier_ties() {
        let record = Record {
            materials: [0x62, ABSENT],
            categories: [0, 3],
            levels: [1, 0],
            flags: [0; 3],
        };
        let gain = VoiceGain {
            authored_pitch: 4096,
            pitch_slot: 1,
            trim: 1.,
            level: 1,
            base_level: 1,
            slot: 18,
        };
        let mut groups = Groups::default();
        let mut world = World::new();
        for stamp in 0..GROUPS as u32 {
            let (index, reset, stolen) = groups
                .allocate(stamp / 2, record, [stamp as f32; 4])
                .unwrap();
            assert_eq!(index, stamp as usize);
            assert!(!reset && stolen.is_empty());
            groups.start(index);
            groups.attach(index, world.spawn_empty().id(), gain);
        }
        assert_eq!(groups.positions().count(), GROUPS);
        let (index, reset, stolen) = groups.allocate(9, record, [0.; 4]).unwrap();
        assert_eq!((index, reset, stolen.len()), (0, true, 1));
        // A reused group restarts its initial playback after evaluation.
        assert!(groups.unstarted().is_empty());
        groups.evaluated();
        assert_eq!(
            groups
                .unstarted()
                .iter()
                .map(|(i, _)| *i)
                .collect::<Vec<_>>(),
            [0]
        );
        groups.start(index);
        groups.attach(index, world.spawn_empty().id(), gain);
        let (index, ..) = groups.allocate(9, record, [0.; 4]).unwrap();
        assert_eq!(index, 1);
        groups.start(index);
        groups.attach(index, world.spawn_empty().id(), gain);
        // A finished voice frees its group for the next record.
        let finished = groups.0[5].voices[0].0;
        assert_eq!(groups.retain(|voice| voice != finished), [5]);
        assert_eq!(groups.positions().count(), GROUPS - 1);
        assert_eq!(groups.allocate(10, record, [0.; 4]).unwrap().0, 5);
        // So does a started record whose sides produced no voice, but not a
        // record still waiting for its initial playback.
        assert!(groups.retain(|_| true).is_empty());
        groups.start(5);
        assert_eq!(groups.retain(|_| true), [5]);
        assert_eq!(groups.allocate(11, record, [0.; 4]).unwrap().0, 5);
        let mut full = Groups::default();
        for _ in 0..GROUPS {
            full.allocate(u32::MAX, record, [0.; 4]).unwrap();
        }
        assert!(full.allocate(1, record, [0.; 4]).is_none());
    }

    #[test]
    fn voice_gain_follows_the_groups_live_volume_slot() {
        let gain = VoiceGain {
            authored_pitch: 4096,
            pitch_slot: 1,
            trim: 0.5,
            level: 32767,
            base_level: 32767,
            slot: 18,
        };
        let full = gain.volume(&FULL_SCALE);
        assert!(full > 0.49 && full <= 0.5);
        // Slot 18 is the low half of word 9; the high half is slot 19.
        let mut words = [0; 16];
        words[9] = 0x7fff_2400;
        assert!((gain.volume(&words) - 0.5 * 9215. / 32767.).abs() < 1e-3);
        words[9] = 0x7fff_0000;
        assert_eq!(gain.volume(&words), 0.);
    }

    fn bank() -> CollisionBank {
        let profile = Profile {
            intensity: Intensity {
                hard: 0.5,
                maximum: 1.,
                medium: 0.25,
                minimum: 0.,
            },
            volume: Volume {
                soft: [[0, 32767]; 3],
                medium: [[0, 32767]; 3],
                hard: [0, 32767],
                hard_layer_scale: 1.,
            },
        };
        let mut profiles = vec![None; ABSENT as usize];
        let mut controls: Vec<Option<Controls>> = (0..ABSENT).map(|_| None).collect();
        for (id, pitch) in [(2, 2096), (0x62, 3796)] {
            profiles[id] = Some(profile);
            controls[id] = Some(Controls {
                name: format!("m{id}"),
                bank: 0,
                events: Matrix {
                    soft: [100 + id as u32, 200 + id as u32, 300],
                    medium: [400 + id as u32, 500, 600],
                    hard: 700,
                },
                authored_base_level: 32767,
                pitch,
                class: 5,
            });
        }
        let mut classes = [0; 95];
        classes[2] = 1;
        let mut assets = Assets::<AudioSource>::default();
        let clip = assets.add(AudioSource {
            bytes: Vec::new().into(),
        });
        let events = [(0, 298), (0, 402), (0, 198)]
            .map(|key| {
                let clips = EventClips {
                    clips: vec![clip.clone()],
                    gains: vec![0.5],
                };
                (key, clips)
            })
            .into_iter()
            .collect();
        CollisionBank {
            materials: Materials::new(profiles, classes).unwrap(),
            settings: BodySettings {
                cooldown: 15,
                torso_bonecrack: [0.7, 1.25],
                head_bonecrack: [0.75, 1.25],
                bonesnap: [0.65, 1.25],
                facehit_soft: [0.05, 0.3],
                facehit_medium: [0.3, 0.55],
            },
            board_cooldown: Some(6),
            grind_bands: Some([0.25, 0.5]),
            controls,
            classes,
            events,
        }
    }

    #[test]
    fn sides_use_counterpart_classes_categories_and_authored_controls() {
        let bank = bank();
        let record = Record {
            materials: [0x62, 2],
            categories: [0, 1],
            levels: [32767, 16384],
            flags: [0; 3],
        };
        // Torso reads the surface's authored class 1 in its soft row; the
        // surface reads the torso's jump-table class 0 in its medium row.
        let torso = bank.side(&record, 0, 0).unwrap();
        assert_eq!((torso.event, torso.material), (298, "m98"));
        assert_eq!(torso.gain.authored_pitch, 3796);
        assert_eq!(torso.gain.pitch_slot, 1);
        let torso_volume = torso.gain.volume(&FULL_SCALE);
        assert!(torso_volume > 0.49 && torso_volume <= 0.5);
        assert_eq!(torso.gain.slot, 18);
        let surface = bank.side(&record, 1, 0).unwrap();
        assert_eq!(surface.event, 402);
        assert_eq!(surface.gain.authored_pitch, 2096);
        assert!(surface.gain.volume(&FULL_SCALE) < torso_volume);
        // An absent counterpart uses class 0; its own side never sounds.
        let alone = Record {
            materials: [0x62, ABSENT],
            ..record
        };
        assert_eq!(bank.side(&alone, 0, 0).unwrap().event, 198);
        assert!(bank.side(&alone, 1, 0).is_none());
        // Category 3 and events without clips stay silent.
        let silent = Record {
            categories: [3, 2],
            ..record
        };
        assert!(bank.side(&silent, 0, 0).is_none());
        assert!(bank.side(&silent, 1, 0).is_none());
    }

    #[test]
    fn older_settings_keep_deck_on_the_provisional_path() {
        let settings: SettingsEntry =
            serde_json::from_str(r#"{"cooldown":15,"bands":[0,0,0,0,0,0,0,0,0,0]}"#).unwrap();
        assert_eq!(settings.board_cooldown, None);
        assert_eq!(settings.grind_bands, None);
    }

    #[test]
    #[ignore = "requires a bank exported from the user's owned base-disc data"]
    fn owned_bank_resolves_every_event_and_generated_record_side() {
        use skate_core::audio::body_impact::{BodyError, BodyFrame, BodyImpacts, REGIONS};
        let root = std::env::var_os("SKATE_OWNED_AUDIO_ROOT")
            .expect("set SKATE_OWNED_AUDIO_ROOT to the export parent");
        let mut assets = Assets::<AudioSource>::default();
        let (bank, _) = CollisionBank::load(Path::new(&root), &mut assets).expect("collision bank");
        assert_eq!(bank.settings.cooldown, 15);
        assert_eq!(bank.board_cooldown, Some(6));
        assert_eq!(bank.grind_bands, Some([0.25, 0.5]));
        assert_eq!(bank.settings.head_bonecrack, [0.75, 1.25]);
        for controls in bank.controls.iter().flatten() {
            let events = controls.events;
            for event in events
                .soft
                .into_iter()
                .chain(events.medium)
                .chain([events.hard])
            {
                assert!(
                    bank.events.contains_key(&(controls.bank, event)),
                    "{} event {event}",
                    controls.name
                );
            }
        }
        let mut records = 0;
        for tag in 0..=0x91 {
            for strength in [0.003, 0.08, 0.3, 0.7, 1.] {
                let frame = BodyFrame {
                    strengths: [strength; REGIONS],
                    surface_tags: [tag; REGIONS],
                    publication_ratio: 1.,
                    state_2a4: false,
                    state_2a5: false,
                    face_contact: strength > 0.5,
                    hall_of_meat: false,
                    hall_of_meat_layers: [false; REGIONS],
                    channel_5: None,
                    owner: (0, false),
                };
                let update = BodyImpacts::default().update(&frame, &bank.materials, &bank.settings);
                // Tag 0x5F selects the empty table entry 0x5E.
                if tag == 0x5f {
                    assert_eq!(update, Err(BodyError::MissingProfile(0x5e)));
                    continue;
                }
                for record in update.unwrap().records {
                    records += 1;
                    for side in 0..2 {
                        let audible =
                            record.materials[side] != ABSENT && record.categories[side] != 3;
                        assert_eq!(bank.side(&record, side, 0).is_some(), audible, "{record:?}");
                    }
                }
                for off_board in [false, true] {
                    let frame = skate_core::audio::deck_impact::DeckFrame {
                        strength,
                        surface_tag: tag,
                        off_board,
                        state_2a4: false,
                        publication_ratio: 1.0,
                        owner: (0, false),
                    };
                    let update = skate_core::audio::deck_impact::DeckImpacts::default()
                        .update(&frame, &bank.materials, bank.board_cooldown.unwrap())
                        .unwrap();
                    for record in update {
                        for side in 0..2 {
                            let audible =
                                record.materials[side] != ABSENT && record.categories[side] != 3;
                            assert_eq!(
                                bank.side(&record, side, 0).is_some(),
                                audible,
                                "{record:?}"
                            );
                        }
                    }
                }
                //Grind bands bypass ordinary material intensity classification.
                for family in 0..=5 {
                    let frame = skate_core::audio::board_contact::GrindPublication {
                        active: true,
                        family,
                        surface_tag: tag as u32,
                        strength,
                    };
                    if tag == 0x5f {
                        continue;
                    } // Native material-table hole.
                    let record = skate_core::audio::grind_start::GrindStarts::default()
                        .input(
                            frame,
                            bank.grind_bands.unwrap(),
                            (0, false),
                            &bank.materials,
                        )
                        .unwrap()
                        .unwrap();
                    for side in 0..2 {
                        assert!(bank.side(&record, side, 0).is_some(), "{record:?}");
                    }
                }
            }
        }
        assert!(records > 1000, "{records}");
    }
}
