use splst_core::{IoSlot, Button, Controllers, ControllerPort};
use crate::keys;

use serde::{Serialize, Deserialize};
use winit::event::{VirtualKeyCode, ElementState};

use std::collections::HashMap;

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

/// Controller settings.
#[derive(Default, Serialize, Deserialize)]
pub struct ControllerConfig {
    #[serde(skip)]
    pub key_map: HashMap<VirtualKeyCode, (IoSlot, Button)>,

    #[serde(skip)]
    controllers: Controllers, 

    slot1: HashMap<Button, VirtualKeyCode>,
    slot2: HashMap<Button, VirtualKeyCode>,

    connection1: Connection,
    connection2: Connection,

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
    pub fn controllers(&self) -> Controllers {
        self.controllers.clone()
    }

    /// Handle key event when closed. Checks if the key is mapped to a button, and in which case
    /// update ['CtrlSettings::controllers'].
    ///
    /// Returns true if the event is captured.
    pub fn handle_key_closed(&mut self, key: VirtualKeyCode, state: ElementState) -> bool {
        self.key_map
            .get(&key)
            .map(|(slot, button)| {
                self.controllers.set_button(
                    *slot, *button, state == ElementState::Pressed,
                );
            })
            .is_some()
    }

    /// Handle a key press if the menu is open, either by the menu being open while running or in
    /// the start menu. It's used when recording key bindings.
    ///
    /// Returns true if the input is captured.
    pub fn handle_key_open(&mut self, key: VirtualKeyCode, state: ElementState) -> bool {
        if state != ElementState::Pressed {
            return false;
        }

        if let Some((slot, button)) = self.recording {
            self.recording = None;

            let bindings = match slot {
                IoSlot::Slot1 => &mut self.slot1,
                IoSlot::Slot2 => &mut self.slot2,
            };

            bindings.insert(button, key);

            // Perforance: This could be avoided assuming it's done when closing the menu.
            self.generate_key_map();

            true
        } else {
            false
        }
    }

    pub fn generate_key_map(&mut self) {
        let s1 = self.slot1
            .iter()
            .map(|(b, k)| (*k, (IoSlot::Slot1, *b)));
        let s2 = self.slot2
            .iter()
            .map(|(b, k)| (*k, (IoSlot::Slot2, *b)));
        let mut map = HashMap::new();
        for (k, v) in s1.chain(s2) {
            if let Some(other) = map.insert(k, v) {
                self.error = Some(format!(
                    "Duplicate key bindings: {} for {} and {} for {} are both bound to {}",
                    other.1,
                    other.0,
                    v.1,
                    v.0,
                    keys::keycode_name(k),
                ));
                return;
            }
        }
        self.key_map = map; 
    }

    fn show_buttons(&mut self, slot: IoSlot, ui: &mut egui::Ui) {
        let bindings = match slot {
            IoSlot::Slot1 => &mut self.slot1,
            IoSlot::Slot2 => &mut self.slot2,
        };

        let mut generate_key_map = false;

        egui::Grid::new("button_grid").show(ui, |ui| {
            BUTTONS
                .iter()
                .zip(Button::NAMES.iter())
                .for_each(|(b, name)| {
                    ui.label(*name);
                    match bindings.get(b) {
                        Some(key) => {
                            ui.label(keys::keycode_name(*key));
                            if ui.button("Unbind").clicked() {
                                generate_key_map = true;
                                self.is_modified = true;
                                bindings.remove(b);
                            }
                        }
                        None => {
                            ui.label("Not Bound");
                            if ui.button("Bind").clicked() {
                                generate_key_map = true;
                                self.is_modified = true;
                                self.recording = Some((slot, *b));
                            }
                        }
                    }
                    ui.end_row();
                });
        });

        if generate_key_map {
            self.generate_key_map();
        }
    }

    pub fn show(&mut self, ui: &mut egui::Ui) {
        ui.add_enabled_ui(self.recording.is_none(), |ui| {
            ui.group(|ui| {
                ui.horizontal(|ui| {
                    ui.selectable_value(&mut self.show_slot, IoSlot::Slot1, "Slot 1");
                    ui.selectable_value(&mut self.show_slot, IoSlot::Slot2, "Slot 2");
                });

                ui.separator();

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
                    let port = match *connection {
                        Connection::Unconnected => ControllerPort::unconnected(),
                        Connection::Virtual => ControllerPort::digital(),
                    };
                    self.controllers[self.show_slot].replace(port);
                }

                ui.separator();

                egui::ScrollArea::new([false, true]).show(ui, |ui| {
                    self.show_buttons(self.show_slot, ui);
                });
            });
        });
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
