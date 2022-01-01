use super::App;
use crate::{cpu::Irq, system::System};
use std::time::Duration;

#[derive(Default)]
pub struct IrqView {
    flags: [(bool, bool); 10],
    trigger: u32,
}

impl App for IrqView {
    fn name(&self) -> &'static str {
        "IRQ View"
    }

    fn update_tick(&mut self, _: Duration, system: &mut System) {
        let state = &mut system.cpu.bus_mut().irq_state;
        self.flags.iter_mut().zip(IRQS).for_each(|(flag, irq)| {
            *flag = (state.is_triggered(irq), state.is_masked(irq));
        });
        state.status |= self.trigger;
        self.trigger = 0;
    }

    fn show(&mut self, ui: &mut egui::Ui) {
        egui::ScrollArea::vertical().show(ui, |ui| {
            egui::Grid::new("irq_grid").striped(true).show(ui, |ui| {
                ui.strong("interrupt");
                ui.strong("active");
                ui.strong("masked");
                ui.end_row();
                self.flags.iter()
                    .zip(IRQ_LABELS)
                    .enumerate()
                    .for_each(|(i, (flag, label))| {
                        ui.label(label);
                        ui.label(if flag.0 { "true" } else { "false" });
                        ui.label(if flag.1 { "true" } else { "false" });
                        if ui.button("trigger").clicked() {
                            self.trigger |= 1 << i;
                        }
                        ui.end_row();
                    });
            });
        });
    }

    fn show_window(&mut self, ctx: &egui::CtxRef, open: &mut bool) {
        egui::Window::new("Interrupt View")
            .open(open)
            .resizable(true)
            .default_width(240.0)
            .default_height(480.0)
            .show(ctx, |ui| {
                self.show(ui);
            });
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
