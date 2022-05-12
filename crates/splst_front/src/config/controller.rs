use splst_core::{IoSlot, Button, Controllers, controller};
use crate::keys;

use serde::{Serialize, Deserialize};
use winit::event::{VirtualKeyCode, ElementState};

use std::collections::HashMap;

/// Controller settings.
#[derive(Default, Serialize, Deserialize)]
pub struct ControllerConfig {
    connection1: Connection,
    connection2: Connection,

    slot1: ButtonBindings,
    slot2: ButtonBindings,

    #[serde(skip)]
    show_slot: IoSlot,

    #[serde(skip)]
    recording: Option<(IoSlot, Button)>,

    #[serde(skip)]
    error: Option<String>,

    #[serde(skip)]
    pub is_modified: bool,
}

impl ControllerConfig {
    /// Get the key map corresponding to the controller settings. It will always generate a new key
    /// map. If generating the key map fails, an empty map will be returned and the menu will show
    /// and error.
    pub fn get_key_map(&mut self) -> HashMap<VirtualKeyCode, (IoSlot, Button)> {
        gen_key_map(&self.slot1, &self.slot2)
            .map_err(|err| self.error = Some(err))
            .unwrap_or_default()
    }

    /// Handle a key press if the menu is open, either by the menu being open while running or in
    /// the start menu. It's used when recording key bindings.
    ///
    /// Returns true if the input is captured.
    pub fn handle_key_event(
        &mut self,
        key_map: &mut HashMap<VirtualKeyCode, (IoSlot, Button)>,
        key: VirtualKeyCode,
        state: ElementState,
    ) -> bool {
        if state != ElementState::Pressed {
            return false;
        }

        if let Some((slot, button)) = self.recording {
            self.recording = None;
            self.is_modified = true;

            let bindings = match slot {
                IoSlot::Slot1 => &mut self.slot1,
                IoSlot::Slot2 => &mut self.slot2,
            };

            bindings.as_slice_mut()[button as usize] = Some(key);

            // Create a new key map or report the error. Also clears the error if generating the
            // key map succeeded.
            self.error = gen_key_map(&self.slot1, &self.slot2)
                .map(|map| *key_map = map)
                .err();

            true
        } else {
            false
        }
    }

    fn show_buttons(&mut self, slot: IoSlot, ui: &mut egui::Ui) {
        let bindings = match slot {
            IoSlot::Slot1 => &mut self.slot1,
            IoSlot::Slot2 => &mut self.slot2,
        };

        egui::Grid::new("button_grid").show(ui, |ui| {
            bindings
                .as_slice_mut()
                .iter_mut()
                .zip(Button::NAMES.iter())
                .zip(BUTTONS.iter())
                .for_each(|((binding, name), button)| {
                    ui.label(*name);

                    let key_name = binding
                        .map(|key| keys::keycode_name(key))
                        .unwrap_or("Unbound");

                    let rebind = egui::Button::new(key_name);
                    if ui.add_sized([60.0, 8.0], rebind).clicked() {
                        self.recording = Some((slot, *button));
                    }

                    ui.end_row();
                });
        });
    }

    /// Update ['Controllers'] from config. It should only be called when the configs could have
    /// changed since it will reset the internal state of the controllers.
    pub fn update_controllers(&self, controllers: &mut Controllers) {
        let connections = [self.connection1, self.connection2];
        for (ctrl, conn) in controllers.iter_mut().zip(connections.iter()) {
            *ctrl = match conn {
                Connection::Unconnected => controller::Port::unconnected(),
                Connection::Virtual => controller::Port::digital(),
            };
        }
    }

    pub fn show(&mut self, controllers: &mut Controllers, ui: &mut egui::Ui) {
        ui.add_enabled_ui(self.recording.is_none(), |ui| {
            ui.horizontal(|ui| {
                ui.selectable_value(&mut self.show_slot, IoSlot::Slot1, "Slot 1");
                ui.selectable_value(&mut self.show_slot, IoSlot::Slot2, "Slot 2");
            });
            
            ui.add_space(10.0);

            let connection = match self.show_slot {
                IoSlot::Slot1 => &mut self.connection1,
                IoSlot::Slot2 => &mut self.connection2,
            };

            let before = *connection;

            egui::ComboBox::from_label("Connection")
                .selected_text(format!("{:?}", connection))
                .show_ui(ui, |ui| {
                    ui.selectable_value(connection, Connection::Unconnected, "Unconnected");
                    ui.selectable_value(connection, Connection::Virtual, "Virtual");
                });

            if before != *connection {
                self.is_modified = true;
                controllers[self.show_slot] = match *connection {
                    Connection::Unconnected => controller::Port::unconnected(),
                    Connection::Virtual => controller::Port::digital(),
                };
            }

            ui.add_space(10.0);

            egui::ScrollArea::new([false, true]).show(ui, |ui| {
                self.show_buttons(self.show_slot, ui);
            });
        });
    }
}

fn gen_key_map(
    slot1: &ButtonBindings,
    slot2: &ButtonBindings,
) -> Result<HashMap<VirtualKeyCode, (IoSlot, Button)>, String> {
    let s1 = slot1
        .as_slice()
        .iter()
        .zip(BUTTONS.iter())
        .filter_map(|(binding, button)| {
            binding.map(|key| (*button, IoSlot::Slot1, key))
        });
    let s2 = slot2
        .as_slice()
        .iter()
        .zip(BUTTONS.iter())
        .filter_map(|(binding, button)| {
            binding.map(|key| (*button, IoSlot::Slot2, key))
        });
    let mut map = HashMap::new();
    for (button, slot, key) in s1.chain(s2) {
        if let Some((slot2, button2)) = map.insert(key, (slot, button)) {
            return Err(format!(
                "Duplicate key bindings: {button} for {slot} and {button2} for {slot2} are both bound to {}",
                keys::keycode_name(key),
            ));
        }
    }
    Ok(map)
}

#[derive(Debug, Copy, Clone, PartialEq, Serialize, Deserialize)]
enum Connection {
    Unconnected,
    Virtual,
    // DualShock,
}

impl Default for Connection {
    fn default() -> Self {
        Connection::Unconnected
    }
}

#[repr(C)]
#[derive(Default, Serialize, Deserialize)]
struct ButtonBindings {
    select: Option<VirtualKeyCode>,  
    l3: Option<VirtualKeyCode>,  
    r3: Option<VirtualKeyCode>,  
    start: Option<VirtualKeyCode>,  
    up: Option<VirtualKeyCode>,  
    right: Option<VirtualKeyCode>,  
    down: Option<VirtualKeyCode>,  
    left: Option<VirtualKeyCode>,  
    l2: Option<VirtualKeyCode>,  
    r2: Option<VirtualKeyCode>,  
    l1: Option<VirtualKeyCode>,  
    r1: Option<VirtualKeyCode>,  
    triangle: Option<VirtualKeyCode>,  
    circle: Option<VirtualKeyCode>,  
    cross: Option<VirtualKeyCode>,  
    square: Option<VirtualKeyCode>,  
}

impl ButtonBindings {
    fn as_slice_mut(&mut self) -> &mut [Option<VirtualKeyCode>] {
        unsafe {
            std::slice::from_raw_parts_mut(
                self as *mut Self as *mut Option<VirtualKeyCode>, 16
            )
        }
    }

    fn as_slice(&self) -> &[Option<VirtualKeyCode>] {
        unsafe {
            std::slice::from_raw_parts(self as *const Self as *const Option<VirtualKeyCode>, 16)
        }
    }
}

const BUTTONS: [Button; Button::COUNT] = [
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
