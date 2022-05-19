use splst_util::{Bit, BitSet};
use super::IoSlot;

use serde::{Serialize, Deserialize};

use std::fmt;
use std::slice::IterMut;
use std::ops::{Index, IndexMut};

/// Game pad connection.
#[derive(Clone)]
pub enum Connection {
    Unconnected,
    Digital(DigitalController),
}

impl Connection {
    pub fn reset(&mut self) {
        match self {
            Connection::Digital(ctrl) => ctrl.reset_state(),
            Connection::Unconnected => (),
        }
    }

    pub fn set_button(&mut self, button: Button, pressed: bool) {
        trace!("{button} {}", if pressed { "pressed" } else { "released" });
        match self {
            Connection::Digital(ctrl) => ctrl.buttons.set_button(button, pressed),
            Connection::Unconnected => (),
        }
    }

    pub fn unconnected() -> Self {
        Connection::Unconnected
    }

    pub fn digital() -> Self {
        Connection::Digital(DigitalController::default())
    }
}

impl Default for Connection {
    fn default() -> Self {
        Connection::Unconnected
    }
}

#[derive(Default)]
pub struct Controllers(pub(super) [Connection; 2]);

impl Controllers {
    pub fn iter_mut(&mut self) -> IterMut<Connection> {
        self.0.iter_mut()
    }
}

impl Index<IoSlot> for Controllers {
    type Output = Connection;

    fn index(&self, idx: IoSlot) -> &Self::Output {
        &self.0[idx as usize]
    }
}

impl IndexMut<IoSlot> for Controllers {
    fn index_mut(&mut self, idx: IoSlot) -> &mut Self::Output {
        &mut self.0[idx as usize]
    }
}

/// The Sony digital controller.
///      ___                      ___
///   __/_L_\__  Digital Pad   __/_R_\__
///  /    _    \--------------/         \
/// |   _| |_   |            |     /\    |
/// |  |_ X _|  |            |  []    () |
/// |    |_|    |  SEL  STA  |     ><    |
/// |\_________/--------------\_________/|
/// |       |                    |       |
/// |      /                      \      |
///  \____/                        \____/
///
/// The original controller sold with the playstation from 1994 to 1997 when the analog controller
/// and dualschock controller became the standard. It's the same layout as the dualschock but
/// without the joypads.
#[derive(Clone, Default)]
pub struct DigitalController {
    pub(super) buttons: ButtonState, 
    pub(super) state: TransferState,
}

impl DigitalController {
    pub fn button_state(&self) -> ButtonState {
        self.buttons.clone()
    }

    pub(super) fn reset_state(&mut self) {
        self.state = TransferState::Idle;
    }

    pub(super) fn transfer(&mut self, val: u8) -> (u8, bool) {
        match self.state {
            // Controller access command.
            TransferState::Idle if val == 0x1 => {
                self.state = TransferState::Ready;
                (0xff, true)
            }
            // Get ID bits 0..7.
            TransferState::Ready if val == 0x42 => {
                self.state = TransferState::IdHigh; 
                (0x41, true)
            }
            // Get ID bits 8..15.
            TransferState::IdHigh => {
                self.state = TransferState::ButtonsLow;
                (0x5a, true)
            }
            // Get Buttons bits 0..7.
            TransferState::ButtonsLow => {
                self.state = TransferState::ButtonsHigh;
                
                // L3 and R3 should always be high since they of course can't be pressed.
                self.buttons.set_button(Button::R3, false);
                self.buttons.set_button(Button::L3, false);

                (self.buttons.0.bit_range(0, 7) as u8, true)
            }
            // Get Buttons bits 8..15.
            TransferState::ButtonsHigh => {
                self.state = TransferState::Idle;

                trace!("controller transfer done");

                (self.buttons.0.bit_range(8, 15) as u8, false)
            }
            _ => (0xff, false)
        }
    }
}

/// Bitmap where each bit represent if a corresponding button is pressed. If the button is pressed
/// the bit is set low and high otherwise.
#[derive(Clone, Copy)]
pub struct ButtonState(u16);

impl Default for ButtonState {
    fn default() -> Self {
        Self(u16::MAX)
    }
}

impl ButtonState {
    /// Set the state of a button.
    pub fn set_button(&mut self, button: Button, pressed: bool) {
        self.0 = self.0.set_bit(button as usize, !pressed);
    }
    
    /// Check that button is pressed, meaning that the corresponding bit is set low.
    pub fn is_pressed(self, button: Button) -> bool {
        !self.0.bit(button as usize)
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy, Hash, PartialEq, Eq)]
pub enum Button {
    Select = 0,
    L3 = 1,
    R3 = 2,
    Start = 3,
    JoyUp = 4,
    JoyRight = 5,
    JoyDown = 6,
    JoyLeft = 7,
    L2 = 8,
    R2 = 9,
    L1 = 10,
    R1 = 11,
    Triangle = 12,
    Circle = 13,
    Cross = 14,
    Square = 15,
}

impl Button {
    /// The number of buttons.
    pub const COUNT: usize = 16;

    /// All of the buttons in order of the corresponding bits in [`ButtonState`].
    pub const ALL: [Button; Self::COUNT] = [
        Button::Select,
        Button::L3,
        Button::R3,
        Button::Start,
        Button::JoyUp,
        Button::JoyRight,
        Button::JoyDown,
        Button::JoyLeft,
        Button::L2,
        Button::R2,
        Button::L1,
        Button::R1,
        Button::Triangle,
        Button::Circle,
        Button::Cross,
        Button::Square,
    ];
}

impl fmt::Display for Button {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(match self {
            Button::Select => "Select",
            Button::L3 => "L3",
            Button::R3 => "R3",
            Button::Start => "Start",
            Button::JoyUp => "Up",
            Button::JoyRight => "Right",
            Button::JoyDown => "Down",
            Button::JoyLeft => "Left",
            Button::L2 => "L2",
            Button::R2 => "R2",
            Button::L1 => "L1",
            Button::R1 => "R1",
            Button::Triangle => "Triangle",
            Button::Circle => "Circle",
            Button::Cross => "Cross",
            Button::Square => "Square",
        })
    }
}

#[derive(Clone, Copy)]
pub(super) enum TransferState {
    Idle,
    Ready,
    /// Get ID high bits.
    IdHigh,
    /// Get buttons low bits.
    ButtonsLow,
    /// Get buttons high bits.
    ButtonsHigh,
}

impl Default for TransferState {
    fn default() -> Self {
        TransferState::Idle
    }
}