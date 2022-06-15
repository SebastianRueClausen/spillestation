use splst_util::{Bit, BitSet};
use super::IoSlot;
use crate::{dump, dump::Dumper};

use serde::{Serialize, Deserialize};

use std::fmt;

pub enum PadKind {
    Digital(DigitalController),
}

impl PadKind {
    pub(super) fn transfer(&mut self, val: u8) -> (u8, bool) {
        match self {
            PadKind::Digital(ctrl) => ctrl.transfer(val),
        }
    }

    pub fn new_digital() -> Self {
        PadKind::Digital(DigitalController::default())
    }

    pub fn set_button(&mut self, button: Button, pressed: bool) {
        match self {
            PadKind::Digital(pad) => pad.button_state().set_button(button, pressed),
        }
    }

    pub fn dump(&self, d: &mut impl Dumper) {
        match self {
            PadKind::Digital(pad) => {
                for (button, name) in Button::ALL.iter().zip(Button::ALL_NAMES.iter()) {
                    let pressed = if pad.button_state().is_pressed(*button) {
                        "✔"
                    } else {
                        " "
                    };

                    dump!(d, *name, "{pressed}");
                }
            }
        }
    }
}

#[derive(Default)]
pub struct GamePads(pub(super) [Option<PadKind>; 2]);

impl GamePads {
    pub fn get(&self, slot: IoSlot) -> &Option<PadKind> {
        &self.0[slot as usize]
    }

    pub fn get_mut(&mut self, slot: IoSlot) -> &mut Option<PadKind> {
        &mut self.0[slot as usize]
    }
    
    pub fn reset_transfer_state(&mut self) {
        self.iter_mut().flatten().for_each(|c| {
            match c {
                PadKind::Digital(pad) => pad.reset_transfer_state(),
            }
        });
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut Option<PadKind>> {
        self.0.iter_mut()
    }
}

/// The Sony digital controller.
///      ___                      ___
///   __/_L_\__  Digital Pad   __/_R_\__
///  /    _    \-------------/         \
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

    fn reset_transfer_state(&mut self) {
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

    pub const ALL_NAMES: [&'static str; Self::COUNT] = [
        "select",
        "l3",
        "r3",
        "start",
        "joyup",
        "joyright",
        "joydown",
        "joyleft",
        "l2",
        "r2",
        "l1",
        "r1",
        "triangle",
        "circle",
        "cross",
        "square",
    ];
}

impl fmt::Display for Button {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(Self::ALL_NAMES[*self as usize])
    }
}

#[derive(Default, Clone, Copy)]
pub(super) enum TransferState {
    #[default]
    Idle,
    Ready,
    /// Get ID high bits.
    IdHigh,
    /// Get buttons low bits.
    ButtonsLow,
    /// Get buttons high bits.
    ButtonsHigh,
}
