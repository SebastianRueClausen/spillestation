use splst_util::{Bit, BitSet};

use serde::{Serialize, Deserialize};

/// The sony digital controller.
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
pub struct DigitalCtrl {
    pub(super) buttons: ButtonState, 
    pub(super) state: TransferState,
}

impl DigitalCtrl {
    pub fn new() -> Self {
        Self {
            buttons: ButtonState::new(),
            state: TransferState::Idle,
        }
    }

    pub fn with_button_state(buttons: ButtonState) -> Self {
        Self {
            buttons,
            state: TransferState::Idle,
        }
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

impl ButtonState {
    fn new() -> Self {
        Self(u16::MAX)
    }

    pub fn set_button(&mut self, button: Button, pressed: bool) {
        self.0 = self.0.set_bit(button as usize, !pressed);
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
