use super::{fps::FrameCounter, cpu::{CpuCtrl, CpuStatus}, mem::MemView, gpu::GpuStatus};
use super::App;
use super::GuiCtx;
use std::time::Duration;
use crate::cpu::Cpu;

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

pub struct AppMenu {
    pub open: bool,
    frame_counter: AppItem<FrameCounter>,
    cpu_ctrl: AppItem<CpuCtrl>,
    cpu_status: AppItem<CpuStatus>,
    mem_view: AppItem<MemView>,
    gpu_status: AppItem<GpuStatus>,
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
        }
    }

    pub fn update_tick(&mut self, dt: Duration, cpu: &mut Cpu) {
        if self.cpu_ctrl.open {
            self.cpu_ctrl.app.run_cpu(dt, cpu); 
        }
        if self.cpu_status.open {
            self.cpu_status.app.update_fields(cpu);
        }
        if self.mem_view.open {
            self.mem_view.app.update_info(cpu.bus());
        }
        if self.gpu_status.open {
            self.gpu_status.app.update_fields(cpu.bus().gpu());
        }
    }

    pub fn draw_tick(&mut self, dt: Duration) {
        self.frame_counter.app.tick(dt);
    }

    pub fn show_apps(&mut self, ctx: &mut GuiCtx) {
        self.cpu_ctrl.show(&ctx.egui_ctx);
        self.cpu_status.show(&ctx.egui_ctx);
        self.mem_view.show(&ctx.egui_ctx);
        self.frame_counter.show(&ctx.egui_ctx);
        self.gpu_status.show(&ctx.egui_ctx);
    }

    pub fn close_apps(&mut self) {
        self.cpu_ctrl.open = false;
        self.cpu_status.open = false;
        self.mem_view.open = false;
        self.frame_counter.open = false;
        self.gpu_status.open = false;
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
                        .max_height(100.0)
                        .auto_shrink([false, false])
                            .show(ui, |ui| {
                                ui.checkbox(&mut self.cpu_ctrl.open, "CPU Control");
                                ui.checkbox(&mut self.cpu_status.open, "CPU Status");
                                ui.checkbox(&mut self.mem_view.open, "Memory View");
                                ui.checkbox(&mut self.frame_counter.open, "Frame Counter");
                                ui.checkbox(&mut self.gpu_status.open, "GPU Status");
                            });
                });
        }
    }
}
