//! The main controller and menu of all the GUI apps. 

use super::{
    fps::FrameCounter,
    cpu::{CpuCtrl, CpuStatus},
    mem::MemView,
    gpu::GpuStatus,
    vram::VramView,
};
use super::App;
use super::GuiCtx;
use std::time::Duration;
use crate::cpu::Cpu;

/// Used to store an application and keep track of the it's open/closed status.
#[derive(Default)]
struct AppItem<T: App + Default> {
    app: T,
    open: bool,
}

impl<T: App + Default> AppItem<T> {
    fn show(&mut self, ctx: &egui::CtxRef) {
        self.app.show(ctx, &mut self.open);
    }
}

/// Controller/Menu for all the apps available. It's responsible for both updating, rendering and
/// controlling when they are visible. It also provides a menu for opening and closing apps.
pub struct AppMenu {
    /// If the menu itself is open.
    pub open: bool,
    frame_counter: AppItem<FrameCounter>,
    cpu_ctrl: AppItem<CpuCtrl>,
    cpu_status: AppItem<CpuStatus>,
    mem_view: AppItem<MemView>,
    gpu_status: AppItem<GpuStatus>,
    vram_view: AppItem<VramView>,
}

impl AppMenu {
    pub fn new() -> Self {
        Self {
            open: false,
            frame_counter: Default::default(),
            cpu_ctrl: Default::default(),
            cpu_status: Default::default(),
            mem_view: Default::default(),
            gpu_status: Default::default(),
            vram_view: Default::default(),
        }
    }

    /// Update all the apps that require it. Called each update cycle.
    pub fn update_tick(&mut self, dt: Duration, cpu: &mut Cpu) {
        // The CPU controller always get's to run.
        self.cpu_ctrl.app.run_cpu(dt, cpu); 
        if self.cpu_status.open {
            self.cpu_status.app.update_fields(cpu);
        }
        if self.mem_view.open {
            self.mem_view.app.update_info(cpu.bus_mut());
        }
        if self.gpu_status.open {
            self.gpu_status.app.update_fields(cpu.bus().gpu());
        }
        if self.vram_view.open {
            self.vram_view.app.update_matrix(cpu.bus().gpu());
        }
    }

    /// Called each frame.
    pub fn draw_tick(&mut self, dt: Duration) {
        self.frame_counter.app.tick(dt);
    }

    /// Show all open apps.
    pub fn show_apps(&mut self, ctx: &mut GuiCtx) {
        self.cpu_ctrl.show(&ctx.egui_ctx);
        self.cpu_status.show(&ctx.egui_ctx);
        self.mem_view.show(&ctx.egui_ctx);
        self.frame_counter.show(&ctx.egui_ctx);
        self.gpu_status.show(&ctx.egui_ctx);
        self.vram_view.show(&ctx.egui_ctx);
    }

    /// Closed all apps. Called if rendering of the GUI has failed.
    pub fn close_apps(&mut self) {
        self.cpu_ctrl.open = false;
        self.cpu_status.open = false;
        self.mem_view.open = false;
        self.frame_counter.open = false;
        self.gpu_status.open = false;
        self.vram_view.open = false;
        self.open = false;
    }

    pub fn show(&mut self, ctx: &egui::CtxRef) {
        if self.open {
            egui::SidePanel::right("App Menu")
                .min_width(4.0)
                .default_width(150.0)
                .show(ctx, |ui| {
                    self.frame_counter.app.update(ui);
                    ui.separator();
                    egui::ScrollArea::vertical()
                        .max_height(400.0)
                        .auto_shrink([false, false])
                            .show(ui, |ui| {
                                ui.checkbox(&mut self.cpu_ctrl.open, "CPU Control");
                                ui.checkbox(&mut self.cpu_status.open, "CPU Status");
                                ui.checkbox(&mut self.mem_view.open, "Memory View");
                                ui.checkbox(&mut self.frame_counter.open, "Frame Counter");
                                ui.checkbox(&mut self.gpu_status.open, "GPU Status");
                                ui.checkbox(&mut self.vram_view.open, "VRAM View");
                            });
                });
        }
    }
}
