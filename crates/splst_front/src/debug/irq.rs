use super::DebugApp;

use splst_core::cpu::Irq;
use splst_core::System;

#[derive(Default)]
pub struct IrqView;

impl DebugApp for IrqView {
    fn name(&self) -> &'static str {
        "IRQ View"
    }

    fn show(&mut self, system: &mut System, ui: &mut egui::Ui) {
        egui::ScrollArea::vertical().show(ui, |ui| {
            egui::Grid::new("irq_grid").show(ui, |ui| {
                ui.label("interrupt");
                ui.label("active");
                ui.label("masked");
                ui.end_row();
                
                let irq_state = system.irq_state();

                for (irq, label) in IRQS.iter().zip(IRQ_LABELS.iter()) {
                    ui.label(*label);
                    ui.label(if irq_state.is_triggered(*irq) { "true" } else { "false" });
                    ui.label(if irq_state.is_masked(*irq) { "true "} else { "false "});
                    ui.end_row();
                }
            });
        });
    }

    fn show_window(&mut self, system: &mut System, ctx: &egui::Context, open: &mut bool) {
        egui::Window::new("Interrupt View")
            .open(open)
            .resizable(true)
            .default_width(240.0)
            .default_height(480.0)
            .show(ctx, |ui| self.show(system, ui));
    }
}

const IRQS: [Irq; 10] = [
    Irq::VBlank,
    Irq::Gpu,
    Irq::CdRom,
    Irq::Dma,
    Irq::Tmr0,
    Irq::Tmr1,
    Irq::Tmr2,
    Irq::CtrlAndMemCard,
    Irq::Sio,
    Irq::Spu,
];

const IRQ_LABELS: [&str; 10] = [
    "VBlank",
    "GPU",
    "CDROM",
    "DMA",
    "TMR0",
    "TMR1",
    "TMR2",
    "CTRL",
    "SIO",
    "SPU",
];
