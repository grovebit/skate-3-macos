//! Optional host selection of samples linked to original material events.
use super::impact::{BodyPart, Contact};
use super::{body_material, wheels};
use bevy::prelude::*;
use serde::Deserialize;
use std::{collections::HashMap, path::Path};

#[derive(Deserialize)]
struct Manifest {
    version: u32,
    #[serde(default)]
    body_events: HashMap<BodyPart, [[BodyEventRoute; 3]; 3]>,
    #[serde(default)]
    surface_classes: Option<Vec<u8>>,
    #[serde(default)]
    body_controls: Option<BodyControlManifest>,
    surfaces: Vec<Route>,
    body: Route,
    #[serde(default)]
    body_parts: HashMap<BodyPart, Route>,
    #[serde(default)]
    wheel_contacts: Option<WheelManifest>,
    #[serde(default)]
    material_intensity: Option<body_material::Tables<body_material::Intensity>>,
    #[serde(default)]
    material_volume: Option<body_material::Tables<body_material::Volume>>,
}
// Ordinary primary-body events only; the scheduler and special-mode overrides
// remain separate. Fixed arrays reject incomplete category/counterpart matrices.
#[derive(Deserialize)]
struct BodyEventRoute {
    bank: String,
    event: u32,
    #[serde(flatten)]
    route: Route,
}
#[derive(Deserialize)]
struct BodyControlManifest {
    materials: HashMap<BodyPart, BodyControls>,
}
/// Authored integers, before any live controller or pitch conversion.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq)]
pub(super) struct BodyControls {
    authored_base_level: i32,
    base_level: i32,
    pitch: i32,
    material_class: i32,
    volume_controller_slot: usize,
}
impl BodyControls {
    fn valid(self) -> bool {
        self.base_level == self.authored_base_level.clamp(0, 32767)
            && self.volume_controller_slot
                == skate_core::audio::voice::volume_slot(self.material_class)
    }
}
struct LoadedBodyEvent {
    event: u32,
    _route: LoadedRoute,
}
struct BodyEventBank {
    classes: [u8; 95],
    parts: HashMap<BodyPart, Vec<Vec<LoadedBodyEvent>>>,
    controls: Option<HashMap<BodyPart, BodyControls>>,
}
#[derive(Debug, PartialEq)]
pub(super) struct BodyEventLayers {
    pub controls: Option<BodyControls>,
    pub primary: Option<(u32, i32)>,
    pub medium_overlay: Option<(u32, i32)>,
}
const PRIMARY_PARTS: [BodyPart; 4] = [
    BodyPart::Head,
    BodyPart::Torso,
    BodyPart::Arm,
    BodyPart::Leg,
];
const BODY_REGIONS: [BodyPart; 6] = [
    BodyPart::Head,
    BodyPart::Torso,
    BodyPart::Arm,
    BodyPart::Arm,
    BodyPart::Leg,
    BodyPart::Leg,
];

#[derive(Deserialize)]
struct WheelManifest {
    thresholds: [f32; 2],
    surface_modes: Vec<bool>,
    modes: Vec<Vec<Option<Route>>>,
    volumes: Vec<Vec<f32>>,
}
struct WheelBank {
    thresholds: [f32; 2],
    surface_modes: Vec<bool>,
    modes: Vec<Vec<Option<LoadedRoute>>>,
    volumes: Vec<Vec<f32>>,
}
#[derive(Deserialize)]
struct Route {
    surface: Option<u8>,
    material: String,
    clips: Vec<String>,
    #[serde(default)]
    gains: Vec<f32>,
}
struct LoadedRoute {
    material: String,
    clips: Vec<Handle<AudioSource>>,
    gains: Vec<f32>,
    paths: Vec<String>,
}
#[derive(Default)]
pub(super) struct Materials {
    surfaces: HashMap<u8, LoadedRoute>,
    body: Option<LoadedRoute>,
    body_parts: HashMap<BodyPart, LoadedRoute>,
    wheels: Option<WheelBank>,
    body_materials: Option<body_material::BodyMaterials>,
    body_events: Option<BodyEventBank>,
}
impl Materials {
    pub fn load(root: &Path, assets: &mut Assets<AudioSource>) -> Self {
        let root = root.join("material-impacts");
        let Some(bytes) = skate_mods::read_bounded(&root, "routes.json", 512 * 1024).ok() else {
            return Self::default();
        };
        let Ok(manifest) = serde_json::from_slice::<Manifest>(&bytes) else {
            warn!("Skating audio: invalid material routes; using generic impacts");
            return Self::default();
        };
        if !valid_manifest(&manifest) {
            warn!("Skating audio: unsupported material routes; using generic impacts");
            return Self::default();
        }
        let body_materials = manifest
            .material_intensity
            .zip(manifest.material_volume)
            .and_then(|(intensity, volume)| body_material::BodyMaterials::load(intensity, volume));
        info!(
            "Skating audio: original body material diagnostic tables loaded={}",
            body_materials.is_some()
        );
        let mut cache = HashMap::new();
        let mut budget = 32 * 1024 * 1024;
        let mut load = |route: Route| {
            let mut clips = Vec::new();
            let mut gains = Vec::new();
            let mut paths = Vec::new();
            for (index, relative) in route.clips.into_iter().enumerate() {
                let clip = cache.entry(relative.clone()).or_insert_with(|| {
                    let bytes = skate_mods::read_bounded(&root, &relative, 2 * 1024 * 1024).ok()?;
                    let (pcm, _) = skate_mods::audio::canonical_pcm_wav(&bytes).ok()?;
                    if pcm.len() > budget {
                        return None;
                    }
                    budget -= pcm.len();
                    Some(assets.add(AudioSource { bytes: pcm.into() }))
                });
                if let Some(clip) = clip {
                    clips.push(clip.clone());
                    gains.push(route.gains.get(index).copied().unwrap_or(1.));
                    paths.push(relative);
                }
            }
            if clips.is_empty() {
                warn!(
                    "Skating audio: no valid impact clips for {}",
                    route.material
                );
                None
            } else {
                Some(LoadedRoute {
                    material: route.material,
                    clips,
                    gains,
                    paths,
                })
            }
        };
        let surfaces = manifest
            .surfaces
            .into_iter()
            .filter_map(|route| {
                let surface = route.surface?;
                load(route).map(|clips| (surface, clips))
            })
            .collect::<HashMap<_, _>>();
        let body = load(manifest.body);
        let body_parts = manifest
            .body_parts
            .into_iter()
            .filter_map(|(part, route)| load(route).map(|route| (part, route)))
            .collect::<HashMap<_, _>>();
        let wheels = manifest.wheel_contacts.and_then(|bank| {
            let modes: Option<Vec<Vec<_>>> = bank
                .modes
                .into_iter()
                .map(|mode| {
                    mode.into_iter()
                        .map(|route| match route {
                            Some(route) => load(route).map(Some),
                            None => Some(None),
                        })
                        .collect()
                })
                .collect();
            Some(WheelBank {
                thresholds: bank.thresholds,
                surface_modes: bank.surface_modes,
                volumes: bank.volumes,
                modes: modes?,
            })
        });
        let body_events = manifest.surface_classes.and_then(|classes| {
            if manifest.body_events.is_empty() {
                return None;
            }
            let parts: Option<HashMap<_, _>> = manifest
                .body_events
                .into_iter()
                .map(|(part, rows)| {
                    let rows: Option<Vec<Vec<_>>> = rows
                        .into_iter()
                        .map(|row| {
                            row.into_iter()
                                .map(|entry| {
                                    load(entry.route).map(|route| LoadedBodyEvent {
                                        event: entry.event,
                                        _route: route,
                                    })
                                })
                                .collect()
                        })
                        .collect();
                    rows.map(|rows| (part, rows))
                })
                .collect();
            Some(BodyEventBank {
                classes: classes.try_into().ok()?,
                parts: parts?,
                controls: manifest.body_controls.map(|controls| controls.materials),
            })
        });
        info!(
            "Skating audio: original body-event bank loaded={}",
            body_events.is_some()
        );
        info!(
            "Skating audio: original wheel-contact tables loaded={}",
            wheels.is_some()
        );
        info!(
            "Skating audio: loaded {} material impact routes, body={}, body_parts={}",
            surfaces.len(),
            body.is_some(),
            body_parts.len()
        );
        Self {
            surfaces,
            body,
            body_parts,
            wheels,
            body_materials,
            body_events,
        }
    }

    pub fn body_levels(&self, strengths: [f32; 6]) -> Option<[body_material::BodyLayers; 6]> {
        self.body_materials
            .as_ref()
            .map(|bank| bank.evaluate(strengths))
    }

    /// Resolve pre-scheduler decisions from matching published region inputs.
    /// These are candidate events, not notifications that a voice was played.
    pub fn body_event_layers(
        &self,
        layers: [body_material::BodyLayers; 6],
        materials: [u32; 6],
    ) -> Option<[BodyEventLayers; 6]> {
        let bank = self.body_events.as_ref()?;
        Some(std::array::from_fn(|i| {
            let class =
                skate_core::audio::material::counterpart_class(materials[i] as i32, &bank.classes)
                    as usize;
            let rows = &bank.parts[&BODY_REGIONS[i]];
            let resolve = |layer: Option<(u8, i32)>| {
                layer.map(|(category, level)| (rows[category as usize][class].event, level))
            };
            BodyEventLayers {
                controls: bank
                    .controls
                    .as_ref()
                    .map(|controls| controls[&BODY_REGIONS[i]]),
                primary: resolve(layers[i].primary),
                medium_overlay: resolve(layers[i].medium_overlay),
            }
        }))
    }

    pub fn has_wheels(&self) -> bool {
        self.wheels.is_some()
    }

    pub fn wheel_input(
        &self,
        touching: [bool; 4],
        air_time: f32,
        airborne: bool,
        surface: u8,
        hardness: f32,
    ) -> Option<wheels::Input> {
        let bank = self.wheels.as_ref()?;
        let mapped = bank.surface_modes[if surface < 94 { surface as usize } else { 94 }];
        Some(wheels::Input {
            touching,
            air_time,
            airborne,
            mode: usize::from(mapped) * 2 + usize::from(hardness < 0.5),
            thresholds: bank.thresholds,
        })
    }

    pub fn select(
        &self,
        contact: Contact,
        variant: usize,
    ) -> Option<(&Handle<AudioSource>, &str, f32, &str)> {
        let (route, volume) = if let Some(event) = contact.wheel {
            let bank = self.wheels.as_ref()?;
            (
                bank.modes.get(event.mode)?.get(event.index)?.as_ref()?,
                *bank.volumes.get(event.gain_mode)?.get(event.index)?,
            )
        } else {
            (
                if let Some(part) = contact.body {
                    self.body_parts.get(&part).or(self.body.as_ref())
                } else {
                    self.surfaces.get(&contact.audio_surface)
                }?,
                1.,
            )
        };
        let index = variant % route.clips.len();
        Some((
            &route.clips[index],
            &route.material,
            route.gains[index] * volume,
            &route.paths[index],
        ))
    }
}

fn valid_manifest(manifest: &Manifest) -> bool {
    let mut seen = [false; 128];
    manifest.version == 1
        && manifest.body_controls.as_ref().is_none_or(|controls| {
            controls.materials.len() == 4
                && PRIMARY_PARTS
                    .iter()
                    .all(|part| controls.materials.contains_key(part))
                && controls.materials.values().all(|controls| controls.valid())
        })
        && manifest
            .surface_classes
            .as_ref()
            .is_none_or(|classes| classes.len() == 95 && classes.iter().all(|c| *c <= 2))
        && (manifest.body_events.is_empty()
            || (manifest.surface_classes.is_some()
                && manifest.body_events.len() == 4
                && PRIMARY_PARTS
                    .iter()
                    .all(|part| manifest.body_events.contains_key(part))
                && manifest
                    .body_events
                    .values()
                    .flatten()
                    .flatten()
                    .all(|entry| {
                        entry.bank == "Skate_Collisions" && entry.route.surface.is_none()
                    })))
        && manifest.wheel_contacts.as_ref().is_none_or(|bank| {
            bank.thresholds.iter().all(|v| v.is_finite())
                && 0. < bank.thresholds[0]
                && bank.thresholds[0] < bank.thresholds[1]
                && bank.thresholds[1] <= 1.
                && bank.surface_modes.len() == 95
                && bank.modes.len() == 4
                && bank.modes.iter().enumerate().all(|(m, mode)| {
                    mode.len() == 13
                        && mode
                            .iter()
                            .enumerate()
                            .all(|(i, route)| route.is_some() || (m > 0 && matches!(i, 2 | 5 | 8)))
                })
                && bank.volumes.len() == 4
                && bank.volumes.iter().all(|mode| {
                    mode.len() == 15 && mode.iter().all(|v| v.is_finite() && *v >= 0. && *v <= 1.)
                })
        })
        && manifest.surfaces.len() <= 128
        && manifest.body.surface.is_none()
        && manifest.body_parts.len() <= 5
        && manifest
            .body_parts
            .values()
            .all(|route| route.surface.is_none())
        && manifest.surfaces.iter().all(|route| {
            route.surface.is_some_and(|id| {
                if id >= 128 || seen[id as usize] {
                    return false;
                }
                seen[id as usize] = true;
                true
            })
        })
        && manifest
            .surfaces
            .iter()
            .chain([&manifest.body])
            .chain(manifest.body_parts.values())
            .chain(
                manifest
                    .wheel_contacts
                    .iter()
                    .flat_map(|bank| bank.modes.iter().flatten().flatten()),
            )
            .chain(
                manifest
                    .body_events
                    .values()
                    .flatten()
                    .flatten()
                    .map(|entry| &entry.route),
            )
            .all(|route| {
                !route.material.is_empty()
                    && route.material.len() <= 80
                    && !route.clips.is_empty()
                    && route.clips.len() <= 4
                    && route.clips.iter().all(|path| path.len() <= 128)
                    && (route.gains.is_empty() || route.gains.len() == route.clips.len())
                    && route
                        .gains
                        .iter()
                        .all(|gain| gain.is_finite() && *gain > 0. && *gain <= 1.)
            })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn route(material: &str, clip: Handle<AudioSource>) -> LoadedRoute {
        LoadedRoute {
            material: material.into(),
            clips: vec![clip],
            gains: vec![1.],
            paths: vec!["test.wav".into()],
        }
    }
    fn event_manifest() -> serde_json::Value {
        use serde_json::json;
        let route = json!({"material":"body", "clips":["hit.wav"]});
        let parts = ["head", "torso", "arm", "leg"];
        let events: serde_json::Map<_, _> = parts
            .iter()
            .enumerate()
            .map(|(part, name)| {
                let rows: Vec<_> = (0..3)
                    .map(|category| {
                        (0..3).map(|class| json!({
                    "bank":"Skate_Collisions", "event":part*100 + category*10 + class,
                    "material":name, "clips":["hit.wav"]
                })).collect::<Vec<_>>()
                    })
                    .collect();
                (name.to_string(), json!(rows))
            })
            .collect();
        let references: serde_json::Map<_, _> = parts
            .iter()
            .map(|name| (name.to_string(), json!("profile")))
            .collect();
        let controls: serde_json::Map<_, _> = parts
            .iter()
            .map(|name| {
                (
                    name.to_string(),
                    json!({
                        "authored_base_level":40000,"base_level":32767,"pitch":3796,
                        "material_class":5,"volume_controller_slot":18
                    }),
                )
            })
            .collect();
        json!({"version":1, "surfaces":[], "body":route,
            "body_controls":{"materials":controls},
            "surface_classes":vec![2;95], "body_events":events,
            "material_intensity":{"materials":references,"tables":{"profile":{
                "minimum":0.125,"medium":0.25,"hard":0.5,"maximum":1.0}}},
            "material_volume":{"materials":references,"tables":{"profile":{
                "soft_0_min":100,"soft_0_max":200,"medium_0_min":300,"medium_0_max":500,
                "hard_min":600,"hard_max":900,"hard_layer_scale":0.5}}}
        })
    }

    #[test]
    fn body_bank_loads_shared_clips_and_resolves_matching_regions_and_layers() {
        let root = std::env::temp_dir().join(format!("skate-body-bank-{}", std::process::id()));
        let folder = root.join("material-impacts");
        std::fs::create_dir_all(&folder).unwrap();
        std::fs::write(folder.join("routes.json"), event_manifest().to_string()).unwrap();
        // One mono PCM sample, shared by every route.
        let mut wav = b"RIFF".to_vec();
        wav.extend_from_slice(&38u32.to_le_bytes());
        wav.extend_from_slice(b"WAVEfmt ");
        wav.extend_from_slice(&16u32.to_le_bytes());
        for value in [1u16, 1] {
            wav.extend_from_slice(&value.to_le_bytes());
        }
        for value in [44100u32, 88200] {
            wav.extend_from_slice(&value.to_le_bytes());
        }
        for value in [2u16, 16] {
            wav.extend_from_slice(&value.to_le_bytes());
        }
        wav.extend_from_slice(b"data");
        wav.extend_from_slice(&2u32.to_le_bytes());
        wav.extend_from_slice(&0i16.to_le_bytes());
        std::fs::write(folder.join("hit.wav"), wav).unwrap();
        let mut assets = Assets::<AudioSource>::default();
        let bank = Materials::load(&root, &mut assets);
        assert_eq!(assets.len(), 1);
        let events = bank
            .body_event_layers(
                bank.body_levels([0.125, 0.375, 0.75, 0.375, 0., 0.125])
                    .unwrap(),
                [0x8f, 0x61, 4, 0x62, 4, 0x60],
            )
            .unwrap();
        assert!(events.iter().all(|layer| layer.controls
            == Some(BodyControls {
                authored_base_level: 40000,
                base_level: 32767,
                pitch: 3796,
                material_class: 5,
                volume_controller_slot: 18,
            })));
        assert_eq!(
            events.map(|layer| (layer.primary, layer.medium_overlay)),
            [
                (Some((0, 100)), None), // Event zero remains a real event.
                (Some((111, 400)), None),
                (Some((222, 750)), Some((212, 375))),
                (Some((210, 400)), None), // Other arm keeps its own counterpart.
                (None, None),
                (Some((302, 100)), None),
            ]
        );
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn body_event_bank_rejects_missing_parts_classes_and_wrong_banks() {
        let valid = |value: &serde_json::Value| {
            serde_json::from_value::<Manifest>(value.clone()).is_ok_and(|m| valid_manifest(&m))
        };
        let original = event_manifest();
        assert!(valid(&original));
        let mut changed = original.clone();
        changed["body_events"]
            .as_object_mut()
            .unwrap()
            .remove("arm");
        assert!(!valid(&changed));
        changed = original.clone();
        changed["surface_classes"] = serde_json::json!(vec![0; 94]);
        assert!(!valid(&changed));
        changed = original.clone();
        changed["surface_classes"][0] = serde_json::json!(3);
        assert!(!valid(&changed));
        changed = original.clone();
        changed["body_events"]["head"][0][0]["bank"] = serde_json::json!("wrong");
        assert!(!valid(&changed));
        changed = original;
        changed["body_events"]["head"][0]
            .as_array_mut()
            .unwrap()
            .pop();
        assert!(!valid(&changed));
    }

    #[test]
    fn body_controls_validate_derived_values_without_requiring_newer_exports() {
        let valid = |value: &serde_json::Value| {
            serde_json::from_value::<Manifest>(value.clone()).is_ok_and(|m| valid_manifest(&m))
        };
        let original = event_manifest();
        for (field, wrong) in [("base_level", 40000), ("volume_controller_slot", 13)] {
            let mut changed = original.clone();
            changed["body_controls"]["materials"]["head"][field] = serde_json::json!(wrong);
            assert!(!valid(&changed));
        }
        let mut changed = original.clone();
        changed["body_controls"]["materials"]
            .as_object_mut()
            .unwrap()
            .remove("head");
        assert!(!valid(&changed));
        changed = original;
        changed.as_object_mut().unwrap().remove("body_controls");
        assert!(valid(&changed));
    }

    #[test]
    #[ignore = "requires a bank exported from the user's owned base-disc data"]
    fn owned_body_event_bank_loads_original_controls_and_all_event_clips() {
        let root = std::env::var_os("SKATE_OWNED_AUDIO_ROOT")
            .expect("set SKATE_OWNED_AUDIO_ROOT to the export parent");
        let mut assets = Assets::<AudioSource>::default();
        let bank = Materials::load(Path::new(&root), &mut assets);
        let loaded = bank
            .body_events
            .as_ref()
            .expect("complete original event bank");
        assert_eq!(loaded.parts.len(), 4);
        for rows in loaded.parts.values() {
            for event in rows.iter().flatten() {
                assert_eq!(event._route.clips.len(), 4);
            }
        }
        let layers = [body_material::BodyLayers {
            primary: Some((2, 750)),
            medium_overlay: Some((1, 375)),
        }; 6];
        let resolved = bank.body_event_layers(layers, [0x60; 6]).unwrap();
        assert_eq!(resolved[1].primary, Some((951, 750)));
        assert_eq!(resolved[1].medium_overlay, Some((950, 375)));
        assert_eq!(resolved[2].primary, Some((1030, 750)));
        assert_eq!(resolved[2].medium_overlay, Some((1030, 375)));
        assert_eq!(resolved[0].controls.unwrap().base_level, 18000);
        assert_eq!(resolved[1].controls.unwrap().pitch, 3796);
        assert!(
            resolved
                .iter()
                .all(|event| event.controls.unwrap().volume_controller_slot == 18)
        );
    }

    #[test]
    fn wheel_tables_select_surface_hardness_and_independent_gain_mode() {
        let clip = Handle::<AudioSource>::default();
        let mut surface_modes = vec![false; 95];
        surface_modes[6] = true;
        surface_modes[94] = true;
        let bank = Materials {
            wheels: Some(WheelBank {
                thresholds: [0.31, 0.5],
                surface_modes,
                modes: (0..4)
                    .map(|_| {
                        (0..13)
                            .map(|_| Some(route("wheel", clip.clone())))
                            .collect()
                    })
                    .collect(),
                volumes: (0..4)
                    .map(|mode| vec![if mode == 3 { 0.4 } else { 0.9 }; 15])
                    .collect(),
            }),
            ..default()
        };
        assert_eq!(
            bank.wheel_input([false; 4], 0., false, 6, 0.49)
                .unwrap()
                .mode,
            3
        );
        assert_eq!(
            bank.wheel_input([false; 4], 0., false, 6, 0.5)
                .unwrap()
                .mode,
            2
        );
        assert_eq!(
            bank.wheel_input([false; 4], 0., false, 3, 0.49)
                .unwrap()
                .mode,
            1
        );
        assert_eq!(
            bank.wheel_input([false; 4], 0., false, 127, 1.)
                .unwrap()
                .mode,
            2
        );
        let selected = bank
            .select(
                Contact {
                    wheel: Some(wheels::Event {
                        index: 2,
                        mode: 0,
                        gain_mode: 3,
                    }),
                    ..default()
                },
                0,
            )
            .unwrap();
        assert_eq!(selected.2, 0.4);
    }
    #[test]
    fn surfaces_and_body_select_their_own_samples_and_unknowns_fall_back() {
        let mut assets = Assets::<AudioSource>::default();
        let concrete = assets.add(AudioSource {
            bytes: vec![1].into(),
        });
        let wood = assets.add(AudioSource {
            bytes: vec![2].into(),
        });
        let body = assets.add(AudioSource {
            bytes: vec![3].into(),
        });
        let mut bank = Materials {
            surfaces: [
                (3, route("concrete", concrete.clone())),
                (41, route("wood", wood.clone())),
            ]
            .into(),
            body: Some(route("torso", body.clone())),
            body_parts: HashMap::new(),
            wheels: None,
            body_materials: None,
            body_events: None,
        };
        let contact = |audio_surface, body: bool| Contact {
            audio_surface,
            body: body.then_some(BodyPart::Torso),
            ..default()
        };
        assert_eq!(bank.select(contact(3, false), 0).unwrap().0, &concrete);
        assert_eq!(
            bank.select(contact(41, false), usize::MAX).unwrap().0,
            &wood
        );
        assert_eq!(bank.select(contact(41, true), 0).unwrap().0, &body);
        assert!(bank.select(contact(127, false), 0).is_none());
        let head_contact = Contact {
            body: Some(BodyPart::Head),
            ..default()
        };
        // Libraries from before body-part routing still provide torso fallback.
        assert_eq!(bank.select(head_contact, 0).unwrap().0, &body);
        let head = assets.add(AudioSource {
            bytes: vec![4].into(),
        });
        bank.body_parts
            .insert(BodyPart::Head, route("head", head.clone()));
        assert_eq!(bank.select(head_contact, 0).unwrap().0, &head);
        let foot_contact = Contact {
            body: Some(BodyPart::Foot),
            ..default()
        };
        assert_eq!(bank.select(foot_contact, 0).unwrap().0, &body);
    }
    #[test]
    fn gain_trims_must_be_attenuation_only_and_match_clip_count() {
        let json = r#"{"version":1,"surfaces":[],"body":{"material":"torso","clips":["b.wav"],"gains":[0.4]}}"#;
        let mut manifest: Manifest = serde_json::from_str(json).unwrap();
        assert!(valid_manifest(&manifest));
        for gains in [vec![1.2], vec![f32::NAN], vec![0.], vec![0.4, 0.4]] {
            manifest.body.gains = gains;
            assert!(!valid_manifest(&manifest));
        }
        manifest.body.gains.clear();
        assert!(valid_manifest(&manifest));
    }

    #[test]
    fn duplicate_or_out_of_range_surfaces_and_empty_routes_are_rejected() {
        let json = r#"{"version":1,"surfaces":[{"surface":3,"material":"concrete","clips":["a.wav"]}],"body":{"material":"torso","clips":["b.wav"]}}"#;
        let mut manifest: Manifest = serde_json::from_str(json).unwrap();
        assert!(valid_manifest(&manifest));
        manifest.surfaces.push(
            serde_json::from_str::<Manifest>(json)
                .unwrap()
                .surfaces
                .remove(0),
        );
        assert!(!valid_manifest(&manifest));
        manifest.surfaces.pop();
        manifest.surfaces[0].surface = Some(255);
        assert!(!valid_manifest(&manifest));
        manifest.surfaces[0].surface = Some(3);
        manifest.body.clips.clear();
        assert!(!valid_manifest(&manifest));
    }
}
