use splst_core::io_port::{pad, IoSlot};
use crate::keys;
use crate::gui::GuiContext;

use winit::event::{VirtualKeyCode, ElementState};

use std::collections::HashMap;

/// Controller settings.
#[derive(Default, serde::Serialize, serde::Deserialize)]
pub struct ControllerConfig {
    #[serde(rename = "connection-1")]
    conn1: Connection,

    #[serde(rename = "connection-2")]
    conn2: Connection,

    #[serde(rename = "slot-1")]
    slot1: ButtonBindings,

    #[serde(rename = "slot-2")]
    slot2: ButtonBindings,

    #[serde(skip)]
    show_slot: IoSlot,

    #[serde(skip)]
    recording: Option<(IoSlot, pad::Button)>,

    #[serde(skip)]
    pub modified: bool,
}

impl ControllerConfig {
    pub fn is_modified(&self) -> bool {
        self.modified
    }

    pub fn mark_as_saved(&mut self) {
        self.modified = false;
    }

    /// Get the key map corresponding to the controller settings. It will always generate a new key
    /// map. If generating the key map fails, an empty map will be returned and the menu will show
    /// and error.
    pub fn get_key_map(
        &mut self,
        ctx: &mut GuiContext
    ) -> HashMap<VirtualKeyCode, (IoSlot, pad::Button)> {
        gen_key_map(&self.slot1, &self.slot2)
            .map_err(|err| ctx.error("Key Error", err))
            .unwrap_or_default()
    }

    /// Handle a key press if the menu is open, either by the menu being open while running or in
    /// the start menu. It's used when recording key bindings.
    ///
    /// Returns true if the input is captured.
    pub fn handle_key_event(
        &mut self,
        key_map: &mut HashMap<VirtualKeyCode, (IoSlot, pad::Button)>,
        key: VirtualKeyCode,
        state: ElementState,
        ctx: &mut GuiContext,
    ) -> bool {
        if state != ElementState::Pressed {
            return false;
        }

        if let Some((slot, button)) = self.recording {
            self.recording = None;
            self.modified = true;

            let bindings = match slot {
                IoSlot::Slot1 => &mut self.slot1,
                IoSlot::Slot2 => &mut self.slot2,
            };

            bindings.as_slice_mut()[button as usize] = Some(key);

            // Create a new key map or report the error. Also clears the error if generating the
            // key map succeeded.
            match gen_key_map(&self.slot1, &self.slot2) {
                Ok(map) => *key_map = map,
                Err(err) => ctx.error("Key Error", err),
            }

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
                .zip(pad::Button::ALL.iter())
                .for_each(|(binding, button)| {
                    ui.label(format!("{button}"));
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

    /// Update [`pad::Controllers`] from config. It should only be called when the configs could
    /// have changed since it will reset the internal state of the controllers.
    pub fn update_controllers(&self, controllers: &mut pad::Controllers) {
        for (ctrl, conn) in controllers.iter_mut().zip([self.conn1, self.conn2].iter()) {
            *ctrl = match conn {
                Connection::Unconnected => pad::Connection::unconnected(),
                Connection::Virtual => pad::Connection::digital(),
            };
        }
    }

    pub fn show(&mut self, controllers: &mut pad::Controllers, ui: &mut egui::Ui) {
        ui.add_enabled_ui(self.recording.is_none(), |ui| {
            ui.horizontal(|ui| {
                ui.selectable_value(&mut self.show_slot, IoSlot::Slot1, "Slot 1");
                ui.selectable_value(&mut self.show_slot, IoSlot::Slot2, "Slot 2");
            });
            
            ui.add_space(10.0);

            let connection = match self.show_slot {
                IoSlot::Slot1 => &mut self.conn1,
                IoSlot::Slot2 => &mut self.conn2,
            };

            let before = *connection;

            egui::ComboBox::from_label("Connection")
                .selected_text(format!("{:?}", connection))
                .show_ui(ui, |ui| {
                    ui.selectable_value(connection, Connection::Unconnected, "Unconnected");
                    ui.selectable_value(connection, Connection::Virtual, "Virtual");
                });

            if before != *connection {
                self.modified = true;
                controllers[self.show_slot] = match *connection {
                    Connection::Unconnected => pad::Connection::unconnected(),
                    Connection::Virtual => pad::Connection::digital(),
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
) -> Result<HashMap<VirtualKeyCode, (IoSlot, pad::Button)>, String> {
    let s1 = slot1
        .as_slice()
        .iter()
        .zip(pad::Button::ALL.iter())
        .filter_map(|(binding, button)| {
            binding.map(|key| (*button, IoSlot::Slot1, key))
        });
    let s2 = slot2
        .as_slice()
        .iter()
        .zip(pad::Button::ALL.iter())
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

#[derive(Debug, Copy, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
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
#[derive(Default, serde::Serialize, serde::Deserialize)]
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
