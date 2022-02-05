//! The main controller and menu of all the GUI apps.

use super::App;
use super::cpu::{CpuCtrl, CpuStatus};
use super::fps::FrameCounter;
use super::gpu::GpuStatus;
use super::mem::MemView;
use super::vram::VramView;
use super::timer::TimerView;
use super::irq::IrqView;
use super::schedule::ScheduleView;

use splst_core::System;
use crate::render::Renderer;
use crate::RunMode;

use std::time::Duration;

/// Controller/Menu for all the apps available. It's responsible for both updating, rendering and
/// controlling when they are visible. It also provides a menu for opening and closing apps.
pub struct AppMenu {
    apps: Vec<(Box<dyn App>, bool)>,
    /// If the menu itself is open.
    pub open: bool,
}

impl AppMenu {
    pub fn new() -> Self {
        Self {
            open: false,
            apps: vec![
                (Box::new(CpuCtrl::default()), false),
                (Box::new(CpuStatus::default()), false),
                (Box::new(MemView::default()), false),
                (Box::new(FrameCounter::default()), false),
                (Box::new(GpuStatus::default()), false),
                (Box::new(VramView::default()), false),
                (Box::new(TimerView::default()), false),
                (Box::new(IrqView::default()), false),
                (Box::new(ScheduleView::default()), false),
            ],
        }
    }

    pub fn toggle_open(&mut self) {
        self.open = !self.open;
    }

    /// Update all the apps that require it. Called each update cycle.
    pub fn update_tick(&mut self, dt: Duration, system: &mut System, renderer: &mut Renderer) {
        for (app, open) in &mut self.apps {
            if *open {
                app.update_tick(dt, system, renderer); 
            }
        }
    }

    /// Called each frame.
    pub fn draw_tick(&mut self, dt: Duration) {
        for (app, open) in &mut self.apps {
            if *open {
                app.frame_tick(dt);
            }
        }
    }

    /// Closed all apps. Called if rendering of the GUI has failed.
    pub fn close_apps(&mut self) {
        for (_, open) in &mut self.apps {
            *open = false;
        }
    }

    pub fn show(&mut self, ctx: &egui::CtxRef, mode: &mut RunMode) {
        if *mode == RunMode::Debug {
            for (app, open) in &mut self.apps {
                if *open {
                    app.show_window(ctx, open);
                }
            }
        }
        if self.open {
            egui::SidePanel::right("App Menu")
                .min_width(4.0)
                .default_width(150.0)
                .show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        ui.selectable_value(mode, RunMode::Debug, "Debug");
                        ui.selectable_value(mode, RunMode::Emulation, "Emulation");
                    });
                    ui.separator();
                    ui.add_enabled_ui(*mode == RunMode::Debug, |ui| {
                        egui::ScrollArea::vertical()
                            .max_height(400.0)
                            .auto_shrink([false, false])
                            .show(ui, |ui| {
                                for (app, open) in &mut self.apps {
                                    ui.checkbox(open, app.name());
                                }
                            });
                    });
                });
        }
    }
}