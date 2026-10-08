//! Device transport. Apple's GameController values are repacked into the
//! XInput layout (`gamepad.rs`), so raw signed axes/trigger bytes reach the TU3
//! converter without Bevy/gilrs deadzones or normalized-axis reconstruction.
use skate_core::input::xbox::XboxState;

mod gamepad;
mod macos;
pub(crate) use macos::{poll, sample_game_controllers};

pub(crate) struct DevicePacket {
    pub number: u32,
    pub state: XboxState,
    pub subtype: u8,
}
