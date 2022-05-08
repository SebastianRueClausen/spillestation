//! The main controller and menu of all the GUI apps.

use splst_core::System;
use crate::RunMode;

use super::DebugApp;
use super::cpu::{CpuCtrl, CpuStatus};
use super::fps::FrameCounter;
use super::gpu::GpuStatus;
use super::mem::MemView;
use super::vram::VramView;
use super::timer::TimerView;
use super::irq::IrqView;
use super::schedule::ScheduleView;
use super::io_port::IoPortView;

use std::time::Duration;

pub struct DebugMenu {
    apps: Vec<(Box<dyn DebugApp>, bool)>,
    pub open: bool,
}

impl DebugMenu {
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
                (Box::new(IoPortView::default()), false),
            ],
        }
    }

    pub fn toggle_open(&mut self) {
        self.open = !self.open;
    }

    /// Update all the apps that require it. Called each update cycle.
    pub fn update_tick(&mut self, dt: Duration, system: &mut System) {
        for (app, open) in &mut self.apps {
            if *open {
                app.update_tick(dt, system); 
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

    pub fn show(&mut self, ctx: &egui::Context, mode: &mut RunMode) {
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
