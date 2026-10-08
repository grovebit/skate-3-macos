//! Original APT HUD rendered independently of the world's resolution scale.
use crate::{hud::apt_scene, config::Config, hud::hud_runtime, physics::SkaterRuntime};
use bevy::{
    asset::{RenderAssetUsages, embedded_asset},
    camera::{RenderTarget, ScalingMode, visibility::RenderLayers},
    mesh::VertexAttributeValues,
    prelude::*,
    render::render_resource::{
        AsBindGroup, BlendState, Extent3d, PrimitiveTopology, RenderPipelineDescriptor, ShaderType,
        TextureDimension, TextureFormat,
    },
    shader::ShaderRef,
    sprite_render::{AlphaMode2d, Material2d, Material2dPlugin, MeshMaterial2d},
    ui_render::UiMaterialPlugin,
    window::PrimaryWindow,
};
use std::{collections::BTreeMap, path::PathBuf};

#[derive(Clone, Copy, Debug, ShaderType)]
struct ColorTransform {
    multiply: Vec4,
    add: Vec4,
}
#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
struct HudMaterial {
    #[uniform(0)]
    color: ColorTransform,
    #[texture(1)]
    #[sampler(2)]
    atlas: Handle<Image>,
}
impl Material2d for HudMaterial {
    fn fragment_shader() -> ShaderRef {
        "embedded://skate3rust/hud/hud_render.wgsl".into()
    }
    fn alpha_mode(&self) -> AlphaMode2d {
        AlphaMode2d::Blend
    }
}
/// The offscreen target already contains RGB multiplied by coverage. Applying
/// ImageNode's straight-alpha blend again suppresses the original soft glow.
#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
struct HudComposite {
    #[texture(0)]
    #[sampler(1)]
    image: Handle<Image>,
}
impl UiMaterial for HudComposite {
    fn fragment_shader() -> ShaderRef {
        "embedded://skate3rust/hud/hud_composite.wgsl".into()
    }
    fn specialize(descriptor: &mut RenderPipelineDescriptor, _: UiMaterialKey<Self>) {
        if let Some(fragment) = &mut descriptor.fragment {
            for target in fragment.targets.iter_mut().flatten() {
                target.blend = Some(BlendState::PREMULTIPLIED_ALPHA_BLENDING);
            }
        }
    }
}
struct Slot {
    entity: Entity,
    mesh: Handle<Mesh>,
    material: Handle<HudMaterial>,
}
#[derive(Resource)]
struct Hud {
    target: Handle<Image>,
    composite: Handle<HudComposite>,
    rebind_after_resize: bool,
    runtime: hud_runtime::Runtime,
    source: serde_json::Value,
    shapes: apt_scene::Shapes,
    textures: BTreeMap<String, Handle<Image>>,
    slots: Vec<Slot>,
    generation: u64,
    failed: bool,
}
/// Same string the scoring HUD draws: language table first, then a readable fallback.
pub(crate) fn display_trick(world: &World, label: &str) -> String {
    if label.is_empty() {
        return String::new();
    }
    localize_trick(
        label,
        world
            .get_resource::<Hud>()
            .map(|hud| &hud.runtime.bindings.movie.text_assets),
    )
}

/// 825E51A0 localizes each authored component before composing a literal.
pub(crate) fn localize_trick(label: &str, assets: Option<&crate::hud::apt_text::TextAssets>) -> String {
    if let Some(literal) = label.strip_prefix('#') {
        return literal.to_owned();
    }
    label
        .split_whitespace()
        .map(|part| {
            let text = assets
                .map(|a| a.localize(part))
                .unwrap_or_else(|| part.to_owned());
            if text.starts_with("ID_") {
                humanize_trick_id(&text)
            } else {
                text
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn humanize_trick_id(id: &str) -> String {
    let rest = id
        .strip_prefix("ID_TRICK_")
        .or_else(|| id.strip_prefix("ID_"))
        .unwrap_or(id);
    rest.split('_')
        .filter(|word| !word.is_empty())
        .map(|word| {
            let lower = word.to_ascii_lowercase();
            let mut chars = lower.chars();
            match chars.next() {
                None => String::new(),
                Some(first) => first.to_ascii_uppercase().to_string() + chars.as_str(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

pub(crate) struct ScoringHudPlugin;
impl Plugin for ScoringHudPlugin {
    fn build(&self, app: &mut App) {
        embedded_asset!(app, "hud_render.wgsl");
        embedded_asset!(app, "hud_composite.wgsl");
        app.add_plugins((
            Material2dPlugin::<HudMaterial>::default(),
            UiMaterialPlugin::<HudComposite>::default(),
        ))
        .add_systems(
            PostStartup,
            setup.after(crate::menu::graphics_menu::PresentationSetup),
        )
        .add_systems(
            FixedUpdate,
            advance
                .after(crate::app::SimulationSet::Physics)
                .run_if(crate::menu::graphics_menu::gameplay_active),
        )
        .add_systems(
            Update,
            (resize_target, reset, render)
                .chain()
                .after(crate::app::FrameSet::Physics),
        );
    }
}
fn setup(
    mut commands: Commands,
    config: Res<Config>,
    skater: Res<SkaterRuntime>,
    cameras: Query<Entity, With<IsDefaultUiCamera>>,
    window: Single<&Window, With<PrimaryWindow>>,
    mut images: ResMut<Assets<Image>>,
    mut composites: ResMut<Assets<HudComposite>>,
) {
    // The startup dependency also applies the presentation system's deferred
    // camera spawn before this query. Without it, setup silently lost the HUD.
    let Ok(output) = cameras.single() else {
        error!("Original HUD requires the presentation camera");
        return;
    };
    let root = std::env::var_os("SKATE_SCORING_HUD_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|| config.asset_root.join("private/hud"));
    let result = (|| -> Result<Hud, String> {
        let source: serde_json::Value = serde_json::from_slice(
            &std::fs::read(root.join("runtime/trickdisplay.json"))
                .map_err(|e| format!("{}: {e}", root.display()))?,
        )
        .map_err(|e| e.to_string())?;
        let runtime = hud_runtime::Runtime::load(&source, skater.scoring.hud_input())?;
        let shapes: apt_scene::Shapes =
            serde_json::from_value(source["shapes"].clone()).map_err(|e| e.to_string())?;
        let mut files = BTreeMap::new();
        for shape in shapes.values().flatten() {
            files.insert(
                shape.texture.rgba.clone(),
                [shape.texture.width, shape.texture.height],
            );
        }
        for font in runtime.bindings.movie.text_assets.fonts.values() {
            files.insert(font.texture.clone(), font.size);
        }
        let mut textures = BTreeMap::new();
        for (path, size) in files {
            let bytes = std::fs::read(root.join(&path)).map_err(|e| format!("HUD {path}: {e}"))?;
            if bytes.len() != size[0] as usize * size[1] as usize * 4 {
                return Err(format!("Invalid HUD texture size {path}"));
            }
            let image = Image::new(
                Extent3d {
                    width: size[0],
                    height: size[1],
                    depth_or_array_layers: 1,
                },
                TextureDimension::D2,
                bytes,
                TextureFormat::Rgba8UnormSrgb,
                RenderAssetUsages::RENDER_WORLD,
            );
            textures.insert(path, images.add(image));
        }
        Ok(Hud {
            target: Handle::default(),
            composite: Handle::default(),
            rebind_after_resize: false,
            runtime,
            source,
            shapes,
            textures,
            slots: Vec::new(),
            generation: 0,
            failed: false,
        })
    })();
    match result {
        Ok(mut hud) => {
            let target = images.add(Image::new_target_texture(
                window.physical_width().max(1),
                window.physical_height().max(1),
                TextureFormat::Rgba8UnormSrgb,
                None,
            ));
            hud.target = target.clone();
            commands.spawn((
                Camera2d,
                Projection::Orthographic(OrthographicProjection {
                    scaling_mode: ScalingMode::Fixed {
                        width: 1280.,
                        height: 720.,
                    },
                    ..OrthographicProjection::default_2d()
                }),
                Camera {
                    order: -1,
                    clear_color: ClearColorConfig::Custom(Color::NONE),
                    ..default()
                },
                RenderTarget::Image(target.clone().into()),
                RenderLayers::layer(31),
                Msaa::Off,
            ));
            hud.composite = composites.add(HudComposite { image: target });
            commands.spawn((
                MaterialNode(hud.composite.clone()),
                UiTargetCamera(output),
                GlobalZIndex(1),
                Pickable::IGNORE,
                Node {
                    position_type: PositionType::Absolute,
                    width: percent(100),
                    height: percent(100),
                    ..default()
                },
            ));
            commands.insert_resource(hud);
            info!("Original scoring HUD loaded from {}", root.display());
        }
        Err(error) => error!(
            "Original scoring HUD could not load from {}: {error}. Run ./play.sh to convert your copy of Skate 3",
            root.display()
        ),
    }
}
// Rasterize at output pixel resolution; retain the original 1280x720 APT
// coordinate space. This avoids a second enlargement of every glyph/glow.
fn resize_target(
    hud: Option<ResMut<Hud>>,
    window: Single<&Window, With<PrimaryWindow>>,
    mut images: ResMut<Assets<Image>>,
    mut composites: ResMut<Assets<HudComposite>>,
) {
    let Some(mut hud) = hud else {
        return;
    };
    // Bevy 0.18's UI material preparation has no dependency on GpuImage
    // preparation. Retry once on the following frame, when the replacement
    // image is available regardless of preparation order during the resize.
    if hud.rebind_after_resize {
        if let Some(material) = composites.get_mut(&hud.composite) {
            material.image = hud.target.clone();
        }
    }
    let size = Extent3d {
        width: window.physical_width().max(1),
        height: window.physical_height().max(1),
        depth_or_array_layers: 1,
    };
    hud.rebind_after_resize = resize_image(
        &mut images,
        &mut composites,
        &hud.target,
        &hud.composite,
        size,
    );
}
fn resize_image(
    images: &mut Assets<Image>,
    composites: &mut Assets<HudComposite>,
    target: &Handle<Image>,
    composite: &Handle<HudComposite>,
    size: Extent3d,
) -> bool {
    if images
        .get(target)
        .is_some_and(|image| image.texture_descriptor.size != size)
    {
        // Assets::get_mut emits Modified even without a write. Doing that
        // every frame recreates the GPU target behind the compositor's
        // cached bind group. Only invalidate the image on a real resize.
        if let Some(image) = images.get_mut(target) {
            image.resize(size);
        }
        // The UI material retains its bind group. A real target resize must
        // reprepare it so it samples the replacement GPU texture view.
        if let Some(material) = composites.get_mut(composite) {
            material.image = target.clone();
        }
        return true;
    }
    false
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hud_target_changes_only_on_resize_and_refreshes_composite() {
        // Asset scheduling only: no render plugin, window or gameplay systems.
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, bevy::asset::AssetPlugin::default()))
            .init_asset::<Image>()
            .init_asset::<HudComposite>();
        let target =
            app.world_mut()
                .resource_mut::<Assets<Image>>()
                .add(Image::new_target_texture(
                    1280,
                    720,
                    TextureFormat::Rgba8UnormSrgb,
                    None,
                ));
        let composite = app
            .world_mut()
            .resource_mut::<Assets<HudComposite>>()
            .add(HudComposite {
                image: target.clone(),
            });
        app.update();
        app.world_mut()
            .resource_mut::<Messages<AssetEvent<Image>>>()
            .clear();
        app.world_mut()
            .resource_mut::<Messages<AssetEvent<HudComposite>>>()
            .clear();
        for (width, height, changed) in
            [(1280, 720, false), (1920, 1080, true), (1920, 1080, false)]
        {
            app.world_mut()
                .resource_scope(|world, mut images: Mut<Assets<Image>>| {
                    let mut composites = world.resource_mut::<Assets<HudComposite>>();
                    resize_image(
                        &mut images,
                        &mut composites,
                        &target,
                        &composite,
                        Extent3d {
                            width,
                            height,
                            depth_or_array_layers: 1,
                        },
                    );
                });
            app.update();
            let images: Vec<_> = app
                .world_mut()
                .resource_mut::<Messages<AssetEvent<Image>>>()
                .drain()
                .collect();
            let materials: Vec<_> = app
                .world_mut()
                .resource_mut::<Messages<AssetEvent<HudComposite>>>()
                .drain()
                .collect();
            assert_eq!(
                images
                    .iter()
                    .any(|e| matches!(e, AssetEvent::Modified { id } if *id == target.id())),
                changed
            );
            assert_eq!(
                materials
                    .iter()
                    .any(|e| matches!(e, AssetEvent::Modified { id } if *id == composite.id())),
                changed
            );
        }
    }
}
fn reset(
    hud: Option<ResMut<Hud>>,
    skater: Res<SkaterRuntime>,
    map: Res<crate::world::map_transition::CurrentMap>,
) {
    let Some(mut hud) = hud else {
        return;
    };
    if hud.generation == map.generation {
        return;
    }
    match hud_runtime::Runtime::load(&hud.source, skater.scoring.hud_input()) {
        Ok(runtime) => {
            hud.runtime = runtime;
            hud.generation = map.generation;
            hud.failed = false;
        }
        Err(error) => {
            error!("Original scoring HUD reset: {error}");
            hud.failed = true;
        }
    }
}
fn advance(
    hud: Option<ResMut<Hud>>,
    skater: Res<SkaterRuntime>,
    map: Res<crate::world::map_transition::CurrentMap>,
) {
    let Some(mut hud) = hud else {
        return;
    };
    if hud.failed || hud.generation != map.generation {
        return;
    }
    if let Err(error) = hud.runtime.update(
        skater.scoring.hud_input(),
        skater.scoring.new_trick,
        skater.scoring.modified_trick,
        skater.scoring.close_tricks,
    ) {
        error!("Original scoring HUD stopped: {error}");
        hud.failed = true;
    }
}
fn hud_position(vertex: &apt_scene::Vertex) -> [f32; 3] {
    [vertex.position[0] - 640., 360. - vertex.position[1], 0.]
}

// Slots own their meshes: topology and normals are fixed by draw_mesh. Compare
// the uploaded positions and UVs before allocating a replacement or emitting
// AssetEvent::Modified. The APT traversal and frame advancement still run.
fn mesh_matches_draw(mesh: &Mesh, draw: &apt_scene::Draw) -> bool {
    let Some(VertexAttributeValues::Float32x3(positions)) =
        mesh.attribute(Mesh::ATTRIBUTE_POSITION)
    else {
        return false;
    };
    let Some(VertexAttributeValues::Float32x2(uvs)) = mesh.attribute(Mesh::ATTRIBUTE_UV_0) else {
        return false;
    };
    positions
        .iter()
        .copied()
        .eq(draw.vertices.iter().map(hud_position))
        && uvs.iter().copied().eq(draw.vertices.iter().map(|v| v.uv))
}

fn draw_mesh(draw: &apt_scene::Draw) -> Mesh {
    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
    );
    mesh.insert_attribute(
        Mesh::ATTRIBUTE_POSITION,
        draw.vertices.iter().map(hud_position).collect::<Vec<_>>(),
    );
    mesh.insert_attribute(
        Mesh::ATTRIBUTE_UV_0,
        draw.vertices.iter().map(|v| v.uv).collect::<Vec<_>>(),
    );
    mesh.insert_attribute(
        Mesh::ATTRIBUTE_NORMAL,
        vec![[0., 0., 1.]; draw.vertices.len()],
    );
    mesh
}

fn update_slot_assets(
    slot: &Slot,
    draw: &apt_scene::Draw,
    material: HudMaterial,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<HudMaterial>,
) {
    if meshes
        .get(&slot.mesh)
        .is_some_and(|mesh| !mesh_matches_draw(mesh, draw))
    {
        *meshes.get_mut(&slot.mesh).unwrap() = draw_mesh(draw);
    }
    if materials.get(&slot.material).is_some_and(|old| {
        old.color.multiply != material.color.multiply
            || old.color.add != material.color.add
            || old.atlas != material.atlas
    }) {
        *materials.get_mut(&slot.material).unwrap() = material;
    }
}

fn render(
    mut commands: Commands,
    hud: Option<ResMut<Hud>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<HudMaterial>>,
) {
    let Some(mut hud) = hud else {
        return;
    };
    let draws = if hud.failed {
        Vec::new()
    } else {
        match apt_scene::draw(&hud.runtime.bindings.movie, &hud.runtime.vm, &hud.shapes) {
            Ok(draws) => draws,
            Err(error) => {
                error!("Original HUD geometry: {error}");
                hud.failed = true;
                Vec::new()
            }
        }
    };
    for (index, draw) in draws.iter().enumerate() {
        let Some(texture) = hud.textures.get(&draw.texture).cloned() else {
            continue;
        };
        let material = HudMaterial {
            color: ColorTransform {
                multiply: draw.multiply.into(),
                add: draw.add.into(),
            },
            atlas: texture,
        };
        if index == hud.slots.len() {
            let mesh = meshes.add(draw_mesh(draw));
            let material = materials.add(material);
            let entity = commands
                .spawn((
                    Mesh2d(mesh.clone()),
                    MeshMaterial2d(material.clone()),
                    Transform::from_xyz(0., 0., index as f32 * 0.01),
                    RenderLayers::layer(31),
                ))
                .id();
            hud.slots.push(Slot {
                entity,
                mesh,
                material,
            });
        } else {
            let slot = &hud.slots[index];
            update_slot_assets(slot, draw, material, &mut meshes, &mut materials);
            commands.entity(slot.entity).insert(Visibility::Visible);
        }
    }
    for slot in &hud.slots[draws.len()..] {
        commands.entity(slot.entity).insert(Visibility::Hidden);
    }
}

#[cfg(test)]
mod upload_tests {
    use super::*;

    fn material(draw: &apt_scene::Draw, atlas: Handle<Image>) -> HudMaterial {
        HudMaterial {
            color: ColorTransform {
                multiply: draw.multiply.into(),
                add: draw.add.into(),
            },
            atlas,
        }
    }

    fn asset_app() -> App {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, bevy::asset::AssetPlugin::default()))
            .init_asset::<Mesh>()
            .init_asset::<HudMaterial>();
        app
    }

    fn update(app: &mut App, slot: &Slot, draw: &apt_scene::Draw, atlas: Handle<Image>) {
        app.world_mut()
            .resource_scope(|world, mut meshes: Mut<Assets<Mesh>>| {
                let mut materials = world.resource_mut::<Assets<HudMaterial>>();
                update_slot_assets(
                    slot,
                    draw,
                    material(draw, atlas),
                    &mut meshes,
                    &mut materials,
                );
            });
    }

    fn modifications(app: &mut App) -> (usize, usize) {
        app.update();
        let meshes = app
            .world_mut()
            .resource_mut::<Messages<AssetEvent<Mesh>>>()
            .drain()
            .filter(|e| matches!(e, AssetEvent::Modified { .. }))
            .count();
        let materials = app
            .world_mut()
            .resource_mut::<Messages<AssetEvent<HudMaterial>>>()
            .drain()
            .filter(|e| matches!(e, AssetEvent::Modified { .. }))
            .count();
        (meshes, materials)
    }

    fn slot(app: &mut App, draw: &apt_scene::Draw, atlas: Handle<Image>) -> Slot {
        Slot {
            entity: Entity::PLACEHOLDER,
            mesh: app
                .world_mut()
                .resource_mut::<Assets<Mesh>>()
                .add(draw_mesh(draw)),
            material: app
                .world_mut()
                .resource_mut::<Assets<HudMaterial>>()
                .add(material(draw, atlas)),
        }
    }

    #[test]
    fn uploads_follow_geometry_color_and_atlas_changes_independently() {
        let mut app = asset_app();
        let mut draw = apt_scene::Draw {
            texture: "atlas".into(),
            vertices: vec![
                apt_scene::Vertex {
                    position: [640., 360.],
                    uv: [0., 0.]
                };
                3
            ],
            multiply: [1.; 4],
            add: [0.; 4],
        };
        let mut atlas = Handle::default();
        let slot = slot(&mut app, &draw, atlas.clone());
        assert_eq!(modifications(&mut app), (0, 0));
        for _ in 0..500 {
            update(&mut app, &slot, &draw, atlas.clone());
            assert_eq!(modifications(&mut app), (0, 0));
        }
        draw.vertices[0].position[0] += 0.125;
        update(&mut app, &slot, &draw, atlas.clone());
        assert_eq!(modifications(&mut app), (1, 0));
        draw.vertices[1].uv[0] += 0.125;
        update(&mut app, &slot, &draw, atlas.clone());
        assert_eq!(modifications(&mut app), (1, 0));
        draw.vertices.extend(draw.vertices.clone());
        update(&mut app, &slot, &draw, atlas.clone());
        assert_eq!(modifications(&mut app), (1, 0));
        draw.multiply[3] = 0.5;
        update(&mut app, &slot, &draw, atlas.clone());
        assert_eq!(modifications(&mut app), (0, 1));
        draw.add[0] = 0.25;
        update(&mut app, &slot, &draw, atlas.clone());
        assert_eq!(modifications(&mut app), (0, 1));
        atlas = Handle::Uuid(
            bevy::asset::uuid::Uuid::from_u128(1),
            std::marker::PhantomData,
        );
        update(&mut app, &slot, &draw, atlas.clone());
        assert_eq!(modifications(&mut app), (0, 1));
        draw.vertices.clear();
        draw.add = [0.; 4];
        update(&mut app, &slot, &draw, atlas.clone());
        assert_eq!(modifications(&mut app), (1, 1));
        update(&mut app, &slot, &draw, atlas);
        assert_eq!(modifications(&mut app), (0, 0));
    }

    #[test]
    #[ignore = "requires SKATE_HUD_TEST_SOURCE pointing to owned trickdisplay.json"]
    fn owned_apt_sequence_preserves_draws_and_reduces_asset_updates() {
        let source: serde_json::Value = serde_json::from_slice(
            &std::fs::read(
                std::env::var_os("SKATE_HUD_TEST_SOURCE").expect("SKATE_HUD_TEST_SOURCE"),
            )
            .unwrap(),
        )
        .unwrap();
        let shapes: apt_scene::Shapes = serde_json::from_value(source["shapes"].clone()).unwrap();
        let mut input = hud_runtime::Input {
            sequence_score: 0,
            line_score: 0,
            sequence_timer: 0,
            line_time: 0.,
            line_capacity: 600.,
            multiplier: 1.,
            clean: true,
            sketchy: false,
            stance: [false; 4],
            trick_name: String::new(),
            trick_metrics: std::array::from_fn(|_| crate::hud::apt_vm::Value::Number(0.)),
            context_tricks: Vec::new(),
        };
        let mut runtime = hud_runtime::Runtime::load(&source, input.clone()).unwrap();
        let mut app = asset_app();
        let mut slots = Vec::new();
        let mut textures = BTreeMap::new();
        let mut old_updates = 0;
        let mut new_updates = (0, 0);
        for frame in 0..600 {
            let new_trick = frame == 120 || frame == 240;
            let close = frame == 360;
            if new_trick {
                input.trick_name = "Kickflip".into();
                input.sequence_score += 100;
            }
            if frame == 240 {
                input.multiplier = 2.;
                input.stance[0] = true;
            }
            if close {
                input.line_score = 400;
                input.sequence_score = 0;
            }
            runtime
                .update(input.clone(), new_trick, false, close)
                .unwrap();
            let draws = apt_scene::draw(&runtime.bindings.movie, &runtime.vm, &shapes).unwrap();
            for (index, draw) in draws.iter().enumerate() {
                let next_id = textures.len() as u128 + 1;
                let atlas = textures
                    .entry(draw.texture.clone())
                    .or_insert_with(|| {
                        Handle::Uuid(
                            bevy::asset::uuid::Uuid::from_u128(next_id),
                            std::marker::PhantomData,
                        )
                    })
                    .clone();
                if index == slots.len() {
                    slots.push(slot(&mut app, draw, atlas));
                } else {
                    old_updates += 1;
                    update(&mut app, &slots[index], draw, atlas.clone());
                }
                let actual = app
                    .world()
                    .resource::<Assets<Mesh>>()
                    .get(&slots[index].mesh)
                    .unwrap();
                let expected = draw_mesh(draw);
                for attr in [
                    Mesh::ATTRIBUTE_POSITION,
                    Mesh::ATTRIBUTE_UV_0,
                    Mesh::ATTRIBUTE_NORMAL,
                ] {
                    assert_eq!(
                        actual.attribute(attr),
                        expected.attribute(attr),
                        "frame {frame}, slot {index}"
                    );
                }
                let actual = app
                    .world()
                    .resource::<Assets<HudMaterial>>()
                    .get(&slots[index].material)
                    .unwrap();
                assert_eq!(actual.color.multiply, Vec4::from_array(draw.multiply));
                assert_eq!(actual.color.add, Vec4::from_array(draw.add));
                assert_eq!(actual.atlas, textures[&draw.texture]);
            }
            let modified = modifications(&mut app);
            new_updates.0 += modified.0;
            new_updates.1 += modified.1;
        }
        eprintln!(
            "HUD_UPLOAD_COUNTS frames=600 previous_each={old_updates} mesh={} material={}",
            new_updates.0, new_updates.1
        );
        assert!(old_updates > 0);
        assert!(new_updates.0 < old_updates);
        assert!(new_updates.1 < old_updates);
    }
}
