//! Platform input adapter; no animation or physics state mutation here.
use crate::app::SimulationSet;
use bevy::prelude::*;

mod controllers;
pub(crate) mod gesture_catalog;
mod gesture_mapping_data;
pub(crate) mod gesture_mapping;
pub(crate) mod gesture_input;
pub(crate) mod platform;
pub(crate) use controllers::{ControllerInput, ControllerStatus, RawInput};
use skate_core::input::tick::TickInput;

#[derive(Resource, Clone, Copy, Debug)]
pub(crate) struct PublishedTickInput(pub TickInput);

impl Default for PublishedTickInput {
    fn default() -> Self {
        Self(TickInput::new(
            0,
            skate_core::input::gameplay_map::GameplayActions::from_values([0.0; 18]),
            false,
        ))
    }
}

pub(crate) struct InputPlugin;
impl Plugin for InputPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ControllerInput>()
            .init_resource::<PublishedTickInput>()
            // GameController state is read on the main thread before anything polls it.
            .add_systems(First, platform::sample_game_controllers)
            .add_systems(PreUpdate, poll_controllers.run_if(crate::menu::graphics_menu::gameplay_active))
            .add_systems(FixedUpdate, publish_actions.in_set(SimulationSet::Input));
    }
}

/// `skate3rust --pad-probe`: prints what each controller slot reports, with no
/// game data. It uses the game's renderer settings, so a window appearing also
/// shows that the GPU backend starts.
pub(crate) fn pad_probe() -> AppExit {
    let mut app = App::new();
    app.add_plugins(
        DefaultPlugins
            .set(WindowPlugin {
                primary_window: Some(Window {
                    title: "Skate 3 Rust Engine: controller probe".into(),
                    ..default()
                }),
                ..default()
            })
            .set(crate::app::render_plugin())
            .disable::<bevy::gilrs::GilrsPlugin>(),
    )
    .add_systems(First, platform::sample_game_controllers)
    .add_systems(Update, print_pads);
    app.run()
}

/// Prints a slot whenever its packet number or connection changes.
fn print_pads(mut shown: Local<[Option<Option<u32>>; 4]>) {
    for (index, shown) in shown.iter_mut().enumerate() {
        let packet = platform::poll(index);
        let key = Some(packet.as_ref().map(|p| p.number));
        if *shown == key {
            continue;
        }
        *shown = key;
        match packet {
            Some(p) => println!(
                "pad {index} #{}: buttons {:#06x} triggers {:?} left {:?} right {:?}",
                p.number, p.state.buttons, p.state.triggers, p.state.left, p.state.right
            ),
            None => println!("pad {index}: Disconnected"),
        }
    }
}

pub(crate) fn poll_controllers(mut input: ResMut<ControllerInput>,config:Res<crate::config::Config>,net:Option<Res<crate::multiplayer::Multiplayer>>,windows:Query<&Window>) {
    let previous = input.status;
    let focused=windows.iter().any(|w|w.focused);
    let active=net.is_some_and(|n|n.active());
    input.collect(std::array::from_fn(|slot| {
        if active && ((!focused && config.multiplayer.controller.is_none()) || config.multiplayer.controller.is_some_and(|selected|selected as usize!=slot)) {
            None
        } else {platform::poll(slot)}
    }));
    for (index, (&before, &after)) in previous.iter().zip(&input.status).enumerate() {
        if before != after {
            match after {
                ControllerStatus::Ready => info!("Controller {index}: raw GameController ready"),
                ControllerStatus::Disconnected => info!("Controller {index}: disconnected"),
                ControllerStatus::Unpolled => {}
            }
        }
    }
}

pub(crate) fn publish_actions(
    mut input: ResMut<ControllerInput>,
    mut published: ResMut<PublishedTickInput>,
    menu: Option<Res<crate::menu::graphics_menu::Menu>>,
    debug: Res<crate::camera::debug_cam::DebugCam>,
    camera: Res<crate::camera::CameraRuntime>,
    mods: Option<Res<crate::modding::Mods>>,
) {
    let blocked = !crate::menu::graphics_menu::gameplay_active(menu) || debug.suppress_gameplay(&camera);
    if blocked {
        input.discard_gameplay();
    }
    input.publish_actions();
    let tick = input.tick_input();
    let mut values=*tick.actions().values();
    if !blocked { crate::modding::override_actions(mods.as_deref(), &mut values); }
    let tick=TickInput::new(tick.tick(),skate_core::input::gameplay_map::GameplayActions::from_values(values),tick.controller_available());
    published.0 = if debug.suppress_gameplay(&camera) {
        TickInput::new(
            tick.tick(),
            skate_core::input::gameplay_map::GameplayActions::from_values([0.0; 18]),
            tick.controller_available(),
        )
    } else {
        tick
    };
}
