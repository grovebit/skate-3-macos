use crate::{
    animation, assets, camera,
    config::Config,
    animation::graph_runtime::StockGraphs,
    input,
    physics::{GamePhysics, PhysicsPlugin, SkaterRuntime},
    diagnostics::verification, world,
};
use bevy::{
    prelude::*,
    render::{
        RenderPlugin,
        settings::{Backends, InstanceFlags, RenderCreation, WgpuFeatures, WgpuSettings},
    },
};
use skate_data::GameAssets;

/// Bevy's defaults plus, on request, the query features that make
/// `RenderDiagnosticsPlugin` report per-pass GPU time.
///
/// Opt-in: a required feature the adapter lacks aborts device creation, and the
/// queries are not free. Without them a pass's cost can only be inferred from
/// invocation counts, which says nothing about how long the pass took. Set
/// `SKATE_GPU_TIMING=1` to get `render/**/elapsed_gpu`.
fn wgpu_features() -> WgpuFeatures {
    let default = WgpuSettings::default().features;
    // Timestamps only: Metal has no pipeline statistics queries.
    if std::env::var_os("SKATE_GPU_TIMING").is_some_and(|v| v != "0") {
        default | WgpuFeatures::TIMESTAMP_QUERY | WgpuFeatures::TIMESTAMP_QUERY_INSIDE_ENCODERS
    } else {
        default
    }
}

pub(crate) fn render_plugin() -> RenderPlugin {
    RenderPlugin {
        render_creation: RenderCreation::Automatic(WgpuSettings {
            backends: Some(Backends::METAL),
            // Metal ignores wgpu's validation flags; empty() turns off wgpu-core's
            // debug labels and its indirect-draw argument validation, which costs
            // GPU work on every indirect draw even in release builds.
            instance_flags: InstanceFlags::empty(),
            features: wgpu_features(),
            ..default()
        }),
        ..default()
    }
}

#[derive(SystemSet, Debug, Hash, PartialEq, Eq, Clone)]
pub(crate) enum FrameSet {
    Assets,
    Physics,
    Animation,
    Verification,
}

#[derive(SystemSet, Debug, Hash, PartialEq, Eq, Clone)]
pub(crate) enum SimulationSet {
    Input,
    Controls,
    Physics,
}

pub(crate) fn build(
    config: Config,
    manifest: GameAssets,
    graphs: StockGraphs,
    physics: GamePhysics,
    skater: SkaterRuntime,
) -> App {
    let retail_scene = config.map.as_ref().is_some_and(|map| crate::render::retail_render::RetailScene::for_map(map));
    let mut app = App::new();
    crate::character::custom_models::register_source(&mut app);
    crate::modding::register_source(&mut app);
    app.add_plugins(
        DefaultPlugins
            .set(AssetPlugin {
                file_path: config.asset_root.to_string_lossy().into_owned(),
                ..default()
            })
            .set(WindowPlugin {
                primary_window: Some(Window {
                    title: config.multiplayer.title.clone().unwrap_or_else(||"Skate 3 Rust Engine".into()),
                    resolution: (1280, 800).into(),
                    // Frames present as soon as they are ready, without
                    // waiting for vblank; the graphics menu's FPS limit caps
                    // the rate instead.
                    present_mode: bevy::window::PresentMode::AutoNoVsync,
                    ..default()
                }),
                ..default()
            })
            .set(render_plugin()).build().disable::<bevy::log::LogPlugin>()
            // Gameplay and menu navigation both read raw GameController state
            // (input/platform). No game system consumes Bevy gamepad
            // events/rumble; its second device backend can stall PreUpdate
            // (70.68 ms in the University capture).
            .disable::<bevy::gilrs::GilrsPlugin>(),
    )
    .insert_resource(bevy::winit::WinitSettings {focused_mode:bevy::winit::UpdateMode::Continuous,unfocused_mode:bevy::winit::UpdateMode::Continuous})
    .insert_resource(config)
    .insert_resource(crate::render::retail_render::RetailScene(retail_scene))
    .insert_resource(assets::AssetManifest(manifest))
    .insert_resource(graphs)
    .insert_resource(physics)
    .insert_resource(skater)
    .configure_sets(
        FixedUpdate,
        (
            SimulationSet::Input,
            SimulationSet::Controls,
            SimulationSet::Physics,
        )
            .chain(),
    )
    .configure_sets(
        Update,
        (
            FrameSet::Assets,
            FrameSet::Physics,
            FrameSet::Animation,
            FrameSet::Verification,
        )
            .chain(),
    )
    .add_plugins(crate::hud::fps_overlay::FpsOverlayPlugin)
    .add_plugins((
        crate::render::retail_render::RetailRenderPlugin,
        input::InputPlugin,
        PhysicsPlugin,
        crate::render::presentation::PresentationPlugin,
        crate::replay::ReplayPlugin,
        assets::GameAssetsPlugin,
        animation::AnimationPlugin,
        world::WorldPlugin,
        crate::world::grind_world::GrindGeometryPlugin,
        camera::CameraPlugin,
        crate::menu::graphics_menu::GraphicsMenuPlugin,
        crate::world::map_transition::MapTransitionPlugin,
        crate::render::render_capacity::RenderCapacityPlugin,
        verification::VerificationPlugin,
        crate::diagnostics::performance::PerformancePlugin,
    ));
    app.add_plugins((crate::hud::session_marker::SessionMarkerPlugin, crate::character::customiser::CustomiserPlugin));
    app.add_plugins(crate::character::custom_models::CustomModelsPlugin);
    app.add_plugins(crate::modding::ModdingPlugin);
    crate::menu::teleport_menu::install(&mut app);
    app.add_plugins(crate::multiplayer::MultiplayerPlugin);
    app.add_plugins(crate::hud::scoring_hud::ScoringHudPlugin);
    app.add_plugins(crate::skating_audio::SkatingAudioPlugin);
    app.add_plugins(crate::camera::debug_cam::DebugCamPlugin);
    app.add_systems(Last, crate::diagnostics::crash_context::sample);
    crate::diagnostics::profiling::install(&mut app);
    app
}
