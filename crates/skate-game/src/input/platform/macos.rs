//! Apple's GameController framework, read on the main thread once per frame
//! and repacked into the XInput layout by `gamepad::pack`.
//!
//! GameController getters have no preconditions beyond a live object; objc2
//! marks every generated binding `unsafe`, hence the blocks below.
use super::{
    DevicePacket,
    gamepad::{self, PadValues},
};
use bevy::ecs::system::NonSendMarker;
use objc2::rc::{Retained, autoreleasepool};
use objc2_game_controller::{
    GCController, GCControllerButtonInput, GCControllerDirectionPad, GCExtendedGamepad,
};
use skate_core::input::xbox::XboxState;
use std::{cell::RefCell, sync::Mutex};

/// XINPUT_DEVSUBTYPE_GAMEPAD; only gamepad profiles are enumerated here.
const GAMEPAD: u8 = 1;

#[derive(Clone, Copy)]
struct Reading {
    number: u32,
    state: XboxState,
}

/// Each slot's latest reading, published by `sample_game_controllers` for `poll`.
static LATEST: Mutex<[Option<Reading>; 4]> = Mutex::new([None; 4]);

thread_local! {
    /// GameController objects stay on the main thread, where sampling runs.
    static SLOTS: RefCell<Slots> = RefCell::new(Slots::new());
}

struct Slots {
    controllers: [Option<Retained<GCController>>; 4],
}

pub(crate) fn poll(index: usize) -> Option<DevicePacket> {
    let reading = LATEST.lock().unwrap_or_else(|poisoned| poisoned.into_inner())[index]?;
    Some(DevicePacket {
        number: reading.number,
        state: reading.state,
        subtype: GAMEPAD,
    })
}

/// Reads every connected controller before anything polls this frame.
/// `NonSendMarker` keeps it on the main thread with the GameController objects.
pub(crate) fn sample_game_controllers(_: NonSendMarker) {
    autoreleasepool(|_| SLOTS.with_borrow_mut(Slots::refresh));
}

impl Slots {
    fn new() -> Self {
        // Keep reading pads while the window is unfocused, which two local
        // instances rely on. The switch exists from macOS 11.3.
        if objc2::available!(macos = 11.3) {
            unsafe { GCController::setShouldMonitorBackgroundEvents(true) };
        }
        Self {
            controllers: Default::default(),
        }
    }

    fn refresh(&mut self) {
        let connected: Vec<_> = unsafe { GCController::controllers() }
            .to_vec()
            .into_iter()
            .filter(|controller| unsafe { controller.extendedGamepad() }.is_some())
            .collect();
        gamepad::assign(&mut self.controllers, &connected);
        let mut latest = LATEST.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        for (slot, controller) in latest.iter_mut().zip(&self.controllers) {
            let previous = *slot;
            *slot = controller
                .as_ref()
                .and_then(|controller| unsafe { controller.extendedGamepad() })
                .map(|pad| {
                    let state = gamepad::pack(&read(&pad));
                    // XInput only advances its packet number when the state changes.
                    let number = match previous {
                        Some(previous) if previous.state == state => previous.number,
                        Some(previous) => previous.number.wrapping_add(1),
                        None => 0,
                    };
                    Reading { number, state }
                });
        }
    }
}

fn pressed(button: Retained<GCControllerButtonInput>) -> bool {
    unsafe { button.isPressed() }
}

/// Options and the stick clicks are absent on some controllers.
fn optional(button: Option<Retained<GCControllerButtonInput>>) -> bool {
    button.is_some_and(pressed)
}

fn stick(stick: Retained<GCControllerDirectionPad>) -> [f32; 2] {
    unsafe { [stick.xAxis().value(), stick.yAxis().value()] }
}

fn read(pad: &GCExtendedGamepad) -> PadValues {
    unsafe {
        let dpad = pad.dpad();
        PadValues {
            dpad: [
                pressed(dpad.up()),
                pressed(dpad.down()),
                pressed(dpad.left()),
                pressed(dpad.right()),
            ],
            start: pressed(pad.buttonMenu()),
            back: optional(pad.buttonOptions()),
            thumbs: [
                optional(pad.leftThumbstickButton()),
                optional(pad.rightThumbstickButton()),
            ],
            shoulders: [pressed(pad.leftShoulder()), pressed(pad.rightShoulder())],
            faces: [
                pressed(pad.buttonA()),
                pressed(pad.buttonB()),
                pressed(pad.buttonX()),
                pressed(pad.buttonY()),
            ],
            triggers: [pad.leftTrigger().value(), pad.rightTrigger().value()],
            left: stick(pad.leftThumbstick()),
            right: stick(pad.rightThumbstick()),
        }
    }
}
