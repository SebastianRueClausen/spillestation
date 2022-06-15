use crate::{gui::Popups, RunMode};
use splst_core::bus::AddrUnit;
use splst_core::cpu::{Cpu, Irq, Opcode};
use splst_core::dump::Dumper;
use splst_core::{debug, StopReason, System};

use std::time::Duration;
use std::{fmt, mem, str};

struct BreakPoint<T> {
    name: String,
    on: T,
}

#[derive(PartialEq, Clone, Copy)]
enum IntDisplayMode {
    Hex,
    Signed,
    Unsigned,
}

impl fmt::Display for IntDisplayMode {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let name = match self {
            IntDisplayMode::Hex => "Hex",
            IntDisplayMode::Signed => "Signed",
            IntDisplayMode::Unsigned => "Unsigned",
        };
        f.write_str(name)
    }
}

/// Represents different kind of values in memory.
#[derive(Clone, Copy)]
enum ValueKind {
    Byte(IntDisplayMode),
    HalfWord(IntDisplayMode),
    Word(IntDisplayMode),
    Ptr,
}

impl ValueKind {
    fn width(&self) -> u32 {
        match self {
            ValueKind::Byte(_) => 1,
            ValueKind::HalfWord(_) => 2,
            ValueKind::Word(_) | ValueKind::Ptr => 4,
        }
    }

    fn is_byte(&self) -> bool {
        matches!(self, ValueKind::Byte(_))
    }

    fn is_half_word(&self) -> bool {
        matches!(self, ValueKind::HalfWord(_))
    }

    fn is_word(&self) -> bool {
        matches!(self, ValueKind::Word(_))
    }

    fn is_ptr(&self) -> bool {
        matches!(self, ValueKind::Ptr)
    }

    /// Check that address pointing at `self` is aligned.
    fn addr_aligned(&self, addr: u32) -> bool {
        addr & (self.width() - 1) == 0
    }

    /// Check that `self` is an integer type (not including pointers).
    fn is_integer(&self) -> bool {
        matches!(
            self,
            ValueKind::Byte(_) | ValueKind::HalfWord(_) | ValueKind::Word(_)
        )
    }

    /// The name of the value kind kind, for instance "Pointer" for [`ValueKind::Ptr`].
    fn name(&self) -> &'static str {
        match self {
            ValueKind::Byte(_) => "Byte",
            ValueKind::HalfWord(_) => "Half Word",
            ValueKind::Word(_) => "Word",
            ValueKind::Ptr => "Pointer",
        }
    }
}

struct WatchPoint {
    name: String,
    kind: ValueKind,
    addr: u32,
}

enum BreakKind {
    Irq(Irq),
    Instruction { addr: u32, op: Opcode },
    Load { addr: u32, val: u32 },
    Store { addr: u32, val: u32 },
}

impl fmt::Display for BreakKind {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use BreakKind::*;
        match self {
            Irq(irq) => write!(f, "triggering {irq}"),
            Instruction { addr, op } => write!(f, "executing `{op}` on {addr:08x}"),
            Load { addr, val } => write!(f, "loading `{val}` on {addr:08x}"),
            Store { addr, val } => write!(f, "storing `{val}` to {addr:08x}"),
        }
    }
}

struct Break {
    name: String,
    kind: BreakKind,
}

#[derive(PartialEq)]
enum ExecuteMode {
    Step,
    Run,
}

struct Debugger {
    instructions: Vec<BreakPoint<u32>>,
    loads: Vec<BreakPoint<u32>>,
    stores: Vec<BreakPoint<u32>>,
    irqs: Vec<BreakPoint<Irq>>,
    watch: Vec<WatchPoint>,
    breaks: Vec<Break>,

    execute_mode: ExecuteMode,
    instruction_hz: u64,
    stepped: bool,
    remainder: Duration,
}

impl Default for Debugger {
    fn default() -> Self {
        Self {
            instructions: Vec::default(),
            loads: Vec::default(),
            stores: Vec::default(),
            irqs: Vec::default(),
            watch: Vec::default(),
            breaks: Vec::default(),

            execute_mode: ExecuteMode::Step,
            instruction_hz: 1,
            stepped: false,
            remainder: Duration::ZERO,
        }
    }
}

impl debug::Debugger for Debugger {
    fn instruction(&mut self, _: &Cpu, addr: u32, op: Opcode) {
        for bp in self.instructions.iter().filter(|bp| bp.on == addr) {
            self.breaks.push(Break {
                name: bp.name.clone(),
                kind: BreakKind::Instruction { addr, op },
            });
        }
    }

    fn load<T: AddrUnit>(&mut self, _: &Cpu, addr: u32, val: T) {
        for bp in self.loads.iter().filter(|bp| bp.on == addr) {
            self.breaks.push(Break {
                name: bp.name.clone(),
                kind: BreakKind::Load {
                    addr,
                    val: val.into(),
                },
            });
        }
    }

    fn store<T: AddrUnit>(&mut self, _: &Cpu, addr: u32, val: T) {
        for bp in self.stores.iter().filter(|bp| bp.on == addr) {
            self.breaks.push(Break {
                name: bp.name.clone(),
                kind: BreakKind::Store {
                    addr,
                    val: val.into(),
                },
            });
        }
    }

    fn irq(&mut self, _: &Cpu, irq: Irq) {
        for bp in self.irqs.iter().filter(|bp| bp.on == irq) {
            self.breaks.push(Break {
                name: bp.name.clone(),
                kind: BreakKind::Irq(irq),
            });
        }
    }

    fn should_break(&mut self) -> bool {
        !self.breaks.is_empty()
    }
}

impl Debugger {
    fn run(&mut self, system: &mut System, dt: Duration) {
        match self.execute_mode {
            ExecuteMode::Run => {
                let time = self.remainder + dt;
                let (remainder, stop) = system.run_debug(self.instruction_hz, time, self);
                self.remainder = remainder;

                if let StopReason::Break = stop {
                    self.execute_mode = ExecuteMode::Step;
                }
            }
            ExecuteMode::Step => {
                if self.stepped {
                    self.stepped = false;
                    system.step_debug(1, self);
                }
            }
        }
    }
}

#[derive(PartialEq)]
enum BreakPointKind {
    Instruction,
    Load,
    Store,
    Irq,
}

impl fmt::Display for BreakPointKind {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use BreakPointKind::*;
        match self {
            Instruction => f.write_str("Instruction"),
            Load => f.write_str("Load"),
            Store => f.write_str("Store"),
            Irq => f.write_str("Interrupt"),
        }
    }
}

struct BreakPointMenu {
    addr_input: String,
    irq_input: Irq,
    kind: BreakPointKind,
}

impl Default for BreakPointMenu {
    fn default() -> Self {
        Self {
            addr_input: String::default(),
            irq_input: Irq::Gpu,
            kind: BreakPointKind::Instruction,
        }
    }
}

impl BreakPointMenu {
    fn show(&mut self, dbg: &mut Debugger, popups: &mut Popups, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            egui::ComboBox::from_id_source("breakpoint_kind")
                .selected_text(self.kind.to_string())
                .show_ui(ui, |ui| {
                    use BreakPointKind::*;

                    ui.selectable_value(&mut self.kind, Instruction, "Instruction");
                    ui.selectable_value(&mut self.kind, Load, "Load");
                    ui.selectable_value(&mut self.kind, Store, "Store");
                    ui.selectable_value(&mut self.kind, Irq, "Interrupt");
                });
            match self.kind {
                BreakPointKind::Instruction | BreakPointKind::Load | BreakPointKind::Store => {
                    if let Some(addr) = show_addr_input(&mut self.addr_input, popups, ui) {
                        let breakpoint = BreakPoint {
                            name: mem::take(&mut self.addr_input),
                            on: addr,
                        };
                        match &self.kind {
                            BreakPointKind::Instruction => dbg.instructions.push(breakpoint),
                            BreakPointKind::Load => dbg.loads.push(breakpoint),
                            BreakPointKind::Store => dbg.stores.push(breakpoint),
                            _ => (),
                        }
                    }
                }
                BreakPointKind::Irq => {
                    egui::ComboBox::from_id_source("irq_add")
                        .selected_text(self.irq_input.to_string())
                        .show_ui(ui, |ui| {
                            for irq in &Irq::ITEMS {
                                ui.selectable_value(&mut self.irq_input, *irq, irq.to_string());
                            }
                        });
                    if ui.button("Add").clicked() {
                        let breakpoint = BreakPoint {
                            name: self.irq_input.to_string(),
                            on: self.irq_input,
                        };
                        dbg.irqs.push(breakpoint);
                    }
                }
            }
        });

        ui.separator();

        fn show_breakpoints<T>(
            breakpoints: &mut Vec<BreakPoint<T>>,
            kind: &str,
            ui: &mut egui::Ui,
        ) {
            breakpoints.retain(|bp| {
                ui.label(&bp.name);
                ui.label(kind);
                let retain = !ui.button("\u{2297}").clicked();
                ui.end_row();
                retain
            });
        }

        egui::ScrollArea::vertical().show(ui, |ui| {
            egui::Grid::new("breakpoint_grid").striped(true).show(ui, |ui| {
                ui.strong("Address");
                ui.strong("Kind");
                ui.end_row();

                show_breakpoints(&mut dbg.instructions, "Instruction", ui);
                show_breakpoints(&mut dbg.loads, "Load", ui);
                show_breakpoints(&mut dbg.stores, "Store", ui);
            });
        });
    }
}

/// Show text input box for addresses.
///
/// Returns `None` if either no address was entered or parsing the address failed, in which case it
/// will show an error via `popups`.
fn show_addr_input(input: &mut String, popups: &mut Popups, ui: &mut egui::Ui) -> Option<u32> {
    ui.add_sized([100.0, 15.0], egui::TextEdit::singleline(input));
    if ui.button("Add").clicked() {
        if let Ok(addr) = u32::from_str_radix(input, 16) {
            Some(addr)
        } else {
            popups.add(
                "Invalid address",
                format!("`{input}` is not a valid address"),
            );
            None
        }
    } else {
        None
    }
}

struct WatchPointMenu {
    addr_input: String,
    int_display_mode: IntDisplayMode,
    value_kind: ValueKind,
}

impl Default for WatchPointMenu {
    fn default() -> Self {
        let int_display_mode = IntDisplayMode::Hex;
        Self {
            addr_input: Default::default(),
            value_kind: ValueKind::Word(int_display_mode),
            int_display_mode,
        }
    }
}

impl WatchPointMenu {
    fn show(&mut self, system: &System, dbg: &mut Debugger, popups: &mut Popups, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            egui::ComboBox::from_id_source("kind")
                .selected_text(self.value_kind.name())
                .show_ui(ui, |ui| {
                    ui.selectable_label(self.value_kind.is_byte(), "Byte").clicked().then(||
                        self.value_kind = ValueKind::Byte(self.int_display_mode)
                    );

                    ui.selectable_label(self.value_kind.is_half_word(), "Byte").clicked().then(||
                        self.value_kind = ValueKind::HalfWord(self.int_display_mode)
                    );

                    ui.selectable_label(self.value_kind.is_word(), "Byte").clicked().then(||
                        self.value_kind = ValueKind::Word(self.int_display_mode)
                    );

                    ui.selectable_label(self.value_kind.is_ptr(), "Byte").clicked().then(||
                        self.value_kind = ValueKind::Ptr
                    );
                });

            if self.value_kind.is_integer() {
                int_display_mode_selector(&mut self.int_display_mode, ui);
            }

            if let Some(addr) = show_addr_input(&mut self.addr_input, popups, ui) {
                if !self.value_kind.addr_aligned(addr) {
                    popups.add(
                        "Invalid Address",
                        format!(
                            "Address {} is incorrectly aligned, must have an alignment of {}",
                            self.addr_input,
                            self.value_kind.width(),
                        ),
                    );
                }
                dbg.watch.push(WatchPoint {
                    kind: self.value_kind,
                    name: mem::take(&mut self.addr_input),
                    addr,
                });
            }
        });

        ui.separator();

        egui::ScrollArea::vertical().show(ui, |ui| {
            egui::Grid::new("watchpoints").striped(true).show(ui, |ui| {
                ui.strong("Address");
                ui.strong("Kind");
                ui.end_row();

                dbg.watch.retain_mut(|point| {
                    use ValueKind::*;

                    ui.label(&point.name);

                    fn format_int(
                        val: impl fmt::Display + fmt::LowerHex,
                        mode: IntDisplayMode,
                    ) -> String {
                        use IntDisplayMode::*;
                        match mode {
                            Signed | Unsigned => val.to_string(),
                            Hex => format!("{val:0x}"),
                        }
                    }

                    let bus = system.bus();

                    let val: Option<String> = match point.kind {
                        Byte(mode) => bus.peek::<u8>(point.addr).map(|val| {
                            if let IntDisplayMode::Signed = mode {
                                format_int(val as i8, mode)
                            } else {
                                format_int(val, mode)
                            }
                        }),
                        HalfWord(mode) => bus.peek::<u16>(point.addr).map(|val| {
                            if let IntDisplayMode::Signed = mode {
                                format_int(val as i16, mode)
                            } else {
                                format_int(val, mode)
                            }
                        }),
                        Word(mode) => bus.peek::<u32>(point.addr).map(|val| {
                            if let IntDisplayMode::Signed = mode {
                                format_int(val as i32, mode)
                            } else {
                                format_int(val, mode)
                            }
                        }),
                        Ptr => bus.peek::<u32>(point.addr).map(|val| format!("{val:08x}")),
                    };

                    ui.label(val.unwrap_or_else(|| "???".to_string()));

                    if let Byte(mode) | HalfWord(mode) | Word(mode) = &mut point.kind {
                        int_display_mode_selector(mode, ui);
                    }

                    !ui.button("\u{2297}").clicked()
                });
            });
        });
    }
}

fn int_display_mode_selector(mode: &mut IntDisplayMode, ui: &mut egui::Ui) {
    egui::ComboBox::from_id_source("int_display_mode")
        .selected_text(mode.to_string())
        .show_ui(ui, |ui| {
            use IntDisplayMode::*;

            ui.selectable_value(mode, Hex, "Hex");
            ui.selectable_value(mode, Signed, "Signed");
            ui.selectable_value(mode, Unsigned, "Unsigned");
        });
}

fn show_executor(dbg: &mut Debugger, ui: &mut egui::Ui) {
    ui.horizontal(|ui| {
        use ExecuteMode::*;

        ui.selectable_value(&mut dbg.execute_mode, Step, "Step");
        ui.selectable_value(&mut dbg.execute_mode, Run, "Run");
    });

    ui.add_space(6.0);

    ui.horizontal(|ui| {
        ui.add(
            egui::Slider::new(&mut dbg.instruction_hz, 1..=30_000_000)
                .text("Instructions Per Second")
                .logarithmic(true)
                .clamp_to_range(true)
                .smart_aim(true),
        );

        dbg.stepped = ui.button("Step").clicked();
    });
}

#[derive(PartialEq)]
enum MemoryDisplayMode {
    Value,
    Ascii,
    Instruction,
}

impl Default for MemoryDisplayMode {
    fn default() -> Self {
        MemoryDisplayMode::Value
    }
}

#[derive(Default)]
struct MemoryMenu {
    display_mode: MemoryDisplayMode,
    addr: u32,
    /// An address to highlight.
    highlight: Option<u32>,
    goto: String,
}

impl MemoryMenu {
    const ROW_COUNT: usize = 8;

    /*
    fn highlight(addr: u32) -> Self {
        Self {
            highlight: Some(addr),
            addr,
            ..Default::default()
        }
    }
    */

    fn show(&mut self, errors: &mut Popups, system: &System, ui: &mut egui::Ui) {
        use MemoryDisplayMode::*;

        ui.horizontal(|ui| {
            ui.selectable_value(&mut self.display_mode, Value, "Value");
            ui.selectable_value(&mut self.display_mode, Instruction, "Instruction");
            ui.selectable_value(&mut self.display_mode, Ascii, "Ascii");
        });

        ui.separator();

        ui.horizontal(|ui| {
            let addr = show_addr_input(&mut self.goto, errors, ui);

            let bytes_per_row = match self.display_mode {
                Value | Instruction => 4,
                Ascii => 16,
            };

            if ui.button("⬆").clicked() {
                self.addr = self.addr.saturating_sub(bytes_per_row);
            }

            if ui.button("⬇").clicked() {
                self.addr = self.addr.saturating_add(bytes_per_row);
            }

            if let Some(addr) = addr {
                self.addr = addr;
            }
        });

        ui.separator();

        // Align start address to the previous multiple of 4.
        let start = self.addr & !3;

        match self.display_mode {
            Instruction => {
                egui::Grid::new("ins_grid")
                    .spacing([0.0, 1.0])
                    .striped(true)
                    .show(ui, |ui| {
                        for row in 0..Self::ROW_COUNT {
                            let addr = start + row as u32 * 4;

                            ui.strong(format!("{addr:06x}\t"));

                            let op = match system.bus().peek::<u32>(addr) {
                                Some(val) => Opcode::new(val).to_string(),
                                None => "???".to_string(),
                            };

                            if Some(addr) == self.highlight {
                                ui.strong(op);
                            } else {
                                ui.label(op);
                            };

                            ui.end_row();
                        }
                    });
            }
            Value => {
                egui::Grid::new("val_grid").spacing([0.0, 1.0]).striped(true).show(ui, |ui| {
                    for row in 0..Self::ROW_COUNT {
                        let addr = start.wrapping_add(row as u32 * 4);
                        let val = system.bus().peek::<u32>(addr);

                        if let Some(val) = val {
                            if ui.button("\u{1f50d}").clicked() {
                                self.addr = val;
                            }
                        }

                        ui.strong(format!("{addr:06x}\t"));

                        const HEX: &[u8] = "0123456789abcdef".as_bytes();

                        match val {
                            Some(val) => {
                                for (i, shift) in [24, 16, 8, 0].iter().enumerate() {
                                    let hex = [
                                        HEX[(val >> shift + 4) as usize & 0xf],
                                        HEX[(val >> shift) as usize & 0xf],
                                    ];

                                    let hex = unsafe { str::from_utf8_unchecked(&hex) };

                                    if Some(addr.wrapping_add(i as u32)) == self.highlight {
                                        ui.strong(hex);
                                    } else {
                                        ui.label(hex);
                                    }
                                }
                            }
                            None => {
                                for i in 0..4 {
                                    if Some(addr.wrapping_add(i as u32)) == self.highlight {
                                        ui.strong("??");
                                    } else {
                                        ui.label("??");
                                    }
                                }
                            }
                        }

                        ui.end_row();
                    }
                });
            }
            Ascii => {
                egui::Grid::new("ascii_grid").striped(true).show(ui, |ui| {
                    for row in 0..Self::ROW_COUNT {
                        let addr = start.wrapping_add(row as u32 * 16);

                        ui.strong(format!("{addr:06x}\t"));

                        // TODO: Show hightlight of char is on address `self.highlight`.
                        let line: String = (0..16)
                            .map(|i| {
                                let c = system
                                    .bus()
                                    .peek::<u8>(addr.wrapping_add(i as u32))
                                    .filter(|val| *val < 128)
                                    .unwrap_or(b'.');
                                char::from(c)
                            })
                            .collect();

                        ui.monospace(line);
                        ui.end_row();
                    }
                });
            }
        }
    }
}

#[derive(Default)]
struct VramMenu {
    /// The first x address.
    x: i32,
    /// The first y address.
    y: i32,
    /// Image of the VRAM.
    image: Option<egui::TextureHandle>,
    /// Scale of which the 'image' should be shown.
    image_scale: f32,
}

impl VramMenu {
    fn show(&mut self, system: &System, ui: &mut egui::Ui) {
        const COLUMN_COUNT: usize = 8;
        const ROW_COUNT: usize = 8;

        ui.horizontal(|ui| {
            ui.add(egui::DragValue::new(&mut self.x).speed(1.0));
            ui.add(egui::DragValue::new(&mut self.y).speed(1.0));
        });

        ui.separator();

        egui::Grid::new("vram_value_grid")
            .striped(true)
            .show(ui, |ui| {
                ui.label("");

                // X-coord row.
                for i in 0..COLUMN_COUNT {
                    ui.strong((self.x + i as i32).to_string());
                }

                ui.end_row();

                for y in 0..ROW_COUNT {
                    // Y-coord column.
                    ui.strong((self.y + y as i32).to_string());

                    for x in 0..COLUMN_COUNT {
                        let val = system
                            .gpu()
                            .vram()
                            .load_16(self.x + x as i32, self.y + y as i32);

                        const HEX: &[u8] = "0123456789abcdef".as_bytes();

                        // One cell which is 16-bits represented as 4 hex chars.
                        let mut cols: [u8; 4] = [0x0; 4];

                        for (col, shift) in cols.iter_mut().zip([12, 8, 4, 0].iter()) {
                            let hex = (val >> shift) & 0xf;
                            *col = HEX[hex as usize];
                        }

                        ui.label(unsafe { std::str::from_utf8_unchecked(&cols) });
                    }

                    ui.end_row();
                }
            });

        ui.separator();

        ui.horizontal(|ui| {
            if ui.button("Dump VRAM").clicked() {
                let raw = system.gpu().vram().to_rgba();
                let image = egui::ColorImage::from_rgba_unmultiplied([1024, 512], &raw);

                self.image = Some(ui.ctx().load_texture("vram", image));
            }

            if self.image.is_some() {
                ui.add(egui::Slider::new(&mut self.image_scale, 0.1..=1.0).text("Scale"));
            }
        });

        if let Some(image) = &self.image {
            ui.image(
                image,
                egui::Vec2::new(1024.0 * self.image_scale, 512.0 * self.image_scale),
            );
        }
    }
}

struct DumpGrid<'a> {
    ui: &'a mut egui::Ui,
}

impl<'a> DumpGrid<'a> {
    fn new(ui: &'a mut egui::Ui) -> Self {
        Self { ui }
    }
}

impl<'a> Dumper for DumpGrid<'a> {
    fn dump_addr_unit(&mut self, label: &'static str, val: impl AddrUnit) {
        self.ui.strong(label);
        self.ui.label(format!("{val:08x}"));
        self.ui.end_row()
    }

    fn dump_string(&mut self, label: &'static str, val: String) {
        self.ui.strong(label);
        self.ui.label(val);
        self.ui.end_row()
    }
}

fn show_cpu(system: &System, ui: &mut egui::Ui) {
    egui::ScrollArea::vertical().show(ui, |ui| {
        egui::Grid::new("registes").striped(true).show(ui, |ui| {
            ui.strong("hi");
            ui.label(system.cpu.hi().to_string());
            ui.end_row();

            ui.strong("lo");
            ui.label(system.cpu.lo().to_string());
            ui.end_row();

            ui.strong("pc");
            ui.label(format!("{:08x}", system.cpu.pc()));
            ui.end_row();

            ui.strong("instruction");
            ui.label(system.cpu.current_instruction().to_string());
            ui.end_row();

            system.cpu.registers().dump(&mut DumpGrid::new(ui));
        });
    });
}

fn show_irq(system: &System, ui: &mut egui::Ui) {
    egui::Grid::new("irq_grid").striped(true).show(ui, |ui| {
        ui.strong("interrupt");
        ui.strong("active");
        ui.strong("masked");
        ui.end_row();

        let irq_state = system.irq_state();

        for irq in Irq::ITEMS.iter() {
            ui.label(irq.to_string());
            ui.label(irq_state.is_triggered(*irq).to_string());
            ui.label(irq_state.is_masked(*irq).to_string());
            ui.end_row();
        }
    });
}

fn show_timers(system: &System, ui: &mut egui::Ui) {
    use splst_core::timer::TimerId::*;

    egui::ScrollArea::vertical().show(ui, |ui| {
        for id in &[Tmr0, Tmr1, Tmr2] {
            egui::CollapsingHeader::new(format!("{id}")).show(ui, |ui| {
                egui::Grid::new(format!("grid_{id}")).striped(true).show(ui, |ui| {
                    system.timers().timer(*id).dump(&mut DumpGrid::new(ui));
                });
            });
        }
    });
}

fn show_io_port(system: &System, ui: &mut egui::Ui) {
    egui::ScrollArea::vertical().show(ui, |ui| {
        let io_port = system.io_port();

        egui::Grid::new("grid").striped(true).show(ui, |ui| {
            io_port.dump(&mut DumpGrid::new(ui));
        });

        ui.separator();

        egui::CollapsingHeader::new("Status Register").show(ui, |ui| {
            egui::Grid::new("stat_grid").striped(true).show(ui, |ui| {
                io_port.status_reg().dump(&mut DumpGrid::new(ui));
            })
        });

        egui::CollapsingHeader::new("Control Register").show(ui, |ui| {
            egui::Grid::new("stat_grid").striped(true).show(ui, |ui| {
                io_port.control_reg().dump(&mut DumpGrid::new(ui));
            })
        });

        egui::CollapsingHeader::new("Mode Register").show(ui, |ui| {
            egui::Grid::new("stat_grid").striped(true).show(ui, |ui| {
                io_port.mode_reg().dump(&mut DumpGrid::new(ui));
            })
        });
    });
}

fn show_schedule(system: &System, ui: &mut egui::Ui) {
    let now = system.schedule().now().time_since_startup();

    egui::Grid::new("time_grid").show(ui, |ui| {
        ui.strong("CPU Cycles");
        ui.label(now.as_cpu_cycles().to_string());
        ui.end_row();

        let (mins, secs, millis) = {
            let dur = now.as_duration();
            (dur.as_secs() / 60, dur.as_secs() % 60, dur.subsec_millis())
        };

        ui.strong("Run Time");
        ui.label(format!("{mins},{secs}.{millis}"));
        ui.end_row();
    });

    ui.separator();

    let mut events: Vec<_> = system
        .schedule()
        .iter_event_entries()
        .map(|entry| {
            let cycles_until = entry
                .ready
                .time_since_startup()
                .saturating_sub(now)
                .as_cpu_cycles()
                .to_string();
            (
                cycles_until,
                entry.mode.to_string(),
                entry.event.to_string(),
            )
        })
        .collect();

    events.sort();

    egui::ScrollArea::vertical()
        .auto_shrink([false, true])
        .show(ui, |ui| {
            egui::Grid::new("schdule_grid").striped(true).show(ui, |ui| {
                ui.strong("Ready");
                ui.strong("Repeat Mode");
                ui.strong("Event");
                ui.end_row();

                for (ready, mode, event) in events.into_iter() {
                    ui.label(ready);
                    ui.label(mode);
                    ui.label(event);
                    ui.end_row();
                }
            });
        });
}

fn show_gpu(system: &System, ui: &mut egui::Ui) {
    egui::ScrollArea::vertical().show(ui, |ui| {
        egui::CollapsingHeader::new("Internal").show(ui, |ui| {
            egui::Grid::new("gpu_state_grid").striped(true).show(ui, |ui| {
                system.gpu().dump(&mut DumpGrid::new(ui));
            });
        });

        egui::CollapsingHeader::new("Status Register").show(ui, |ui| {
            egui::Grid::new("gpu_state_grid").striped(true).show(ui, |ui| {
                system.gpu().status().dump(&mut DumpGrid::new(ui));
            });
        });
    });
}

fn show_dma(system: &System, ui: &mut egui::Ui) {
    egui::ScrollArea::vertical().show(ui, |ui| {
        egui::CollapsingHeader::new("Interrupt Register").show(ui, |ui| {
            egui::Grid::new("interrupt register").striped(true).show(ui, |ui| {
                system.dma().irq_reg().dump(&mut DumpGrid::new(ui));
            });
        });

        ui.separator();  

        use splst_core::bus::dma::Port;

        egui::CollapsingHeader::new("MDEC in").show(ui, |ui| {
            egui::Grid::new("channel status").striped(true).show(ui, |ui| {
                system.dma()[Port::MdecIn].dump(&mut DumpGrid::new(ui));
            });
        });

        egui::CollapsingHeader::new("MDEC out").show(ui, |ui| {
            egui::Grid::new("channel status").striped(true).show(ui, |ui| {
                system.dma()[Port::MdecOut].dump(&mut DumpGrid::new(ui));
            });
        });

        egui::CollapsingHeader::new("GPU").show(ui, |ui| {
            egui::Grid::new("channel status").striped(true).show(ui, |ui| {
                system.dma()[Port::Gpu].dump(&mut DumpGrid::new(ui));
            });
        });

        egui::CollapsingHeader::new("CD-ROM").show(ui, |ui| {
            egui::Grid::new("channel status").striped(true).show(ui, |ui| {
                system.dma()[Port::CdRom].dump(&mut DumpGrid::new(ui));
            });
        });

        egui::CollapsingHeader::new("SPU").show(ui, |ui| {
            egui::Grid::new("channel status").striped(true).show(ui, |ui| {
                system.dma()[Port::Spu].dump(&mut DumpGrid::new(ui));
            });
        });

        egui::CollapsingHeader::new("PIO").show(ui, |ui| {
            egui::Grid::new("channel status").striped(true).show(ui, |ui| {
                system.dma()[Port::Pio].dump(&mut DumpGrid::new(ui));
            });
        });

        egui::CollapsingHeader::new("OTC").show(ui, |ui| {
            egui::Grid::new("channel status").striped(true).show(ui, |ui| {
                system.dma()[Port::Otc].dump(&mut DumpGrid::new(ui));
            });
        });
    });
}

fn show_gte(system: &System, ui: &mut egui::Ui) {
    egui::ScrollArea::vertical().show(ui, |ui| {
        egui::CollapsingHeader::new("Data Registers").show(ui, |ui| {
            egui::Grid::new("status").striped(true).show(ui, |ui| {
                system.cpu.gte().data_regs().dump(&mut DumpGrid::new(ui));
            });
        });

        egui::CollapsingHeader::new("Control Registers").show(ui, |ui| {
            egui::Grid::new("status").striped(true).show(ui, |ui| {
                system.cpu.gte().control_regs().dump(&mut DumpGrid::new(ui));
            });
        });
    });
}

fn show_cdrom(system: &System, ui: &mut egui::Ui) {
    egui::ScrollArea::vertical().show(ui, |ui| {
        egui::Grid::new("status").striped(true).show(ui, |ui| {
            system.cdrom().dump(&mut DumpGrid::new(ui));
        });
    });
}

pub struct DebugMenu {
    pub open: bool,
    debugger: Debugger,

    popups: Popups,

    breakpoint: (BreakPointMenu, bool),
    watchpoint: (WatchPointMenu, bool),

    /// Open flag for executor menu.
    executor_open: bool,

    /// Open flag for each of `STATELESS_MENUS`.
    stateless_open: [bool; 9],

    // Allow to have multiple memory and vram menues open.
    memory: Vec<MemoryMenu>,
    vram: Vec<VramMenu>,
}

impl Default for DebugMenu {
    fn default() -> Self {
        Self {
            open: false,
            debugger: Debugger::default(),
            popups: Popups::new("debug"),
            breakpoint: (BreakPointMenu::default(), false),
            watchpoint: (WatchPointMenu::default(), false),
            executor_open: false,
            stateless_open: [false; 9],
            memory: Vec::default(),
            vram: Vec::default(),
        }
    }
}

impl DebugMenu {
    pub fn toggle_open(&mut self) {
        self.open = !self.open;
    }

    pub fn run_debugger(&mut self, dt: Duration, system: &mut System) {
        self.debugger.run(system, dt);
    }

    pub fn show(&mut self, ctx: &egui::Context, system: &mut System, mode: &mut RunMode) {
        for br in self.debugger.breaks.drain(..) {
            self.popups
                .add(format!("Hit {}", br.name), format!("Broke {}", br.kind));
        }

        if let RunMode::Debug = mode {
            if let (menu, open @ true) = &mut self.breakpoint {
                egui::Window::new("Breakpoints").open(open).show(
                    ctx,
                    |ui| {
                        menu.show(&mut self.debugger, &mut self.popups, ui);
                    },
                );
            }

            if let (menu, open @ true) = &mut self.watchpoint {
                egui::Window::new("Watchpoints").open(open).show(
                    ctx,
                    |ui| {
                        menu.show(system, &mut self.debugger, &mut self.popups, ui);
                    },
                );
            }

            if self.executor_open {
                egui::Window::new("Executor")
                    .open(&mut self.executor_open)
                    .show(ctx, |ui| show_executor(&mut self.debugger, ui));
            }

            for ((name, show), open) in STATELESS_MENUS
                .iter()
                .zip(self.stateless_open.iter_mut())
            {
                if *open {
                    egui::Window::new(*name)
                        .open(open)
                        .show(ctx, |ui| show(system, ui));
                }
            }

            let mut i = 0;

            self.memory.retain_mut(|memory| {
                let mut open = true;

                egui::Window::new(format!("Memory {i}"))
                    .open(&mut open)
                    .show(ctx, |ui| memory.show(&mut self.popups, system, ui));

                i += 1;

                open
            });

            i = 0;

            self.vram.retain_mut(|vram| {
                let mut open = true;

                egui::Window::new(format!("VRAM {i}"))
                    .open(&mut open)
                    .show(ctx, |ui| vram.show(system, ui));

                i += 1;

                open
            });

            self.popups.show(ctx);
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
                                ui.checkbox(&mut self.breakpoint.1, "Breakpoints");
                                ui.checkbox(&mut self.watchpoint.1, "Watchpoints");
    
                                ui.checkbox(&mut self.executor_open, "Executor");

                                STATELESS_MENUS
                                    .iter()
                                    .zip(self.stateless_open.iter_mut())
                                    .for_each(|((name, _), open)| {
                                        ui.checkbox(open, *name);
                                    });

                                ui.separator();

                                ui.horizontal(|ui| {
                                    if ui.button("Memory").clicked() {
                                        self.memory.push(MemoryMenu::default());
                                    }
                                    if ui.button("VRAM").clicked() {
                                        self.vram.push(VramMenu::default());
                                    }
                                });
                            });
                    });
                });
        }
    }
}

const STATELESS_MENUS: [(&str, fn(&System, &mut egui::Ui)); 9] = [
    ("CPU", show_cpu),
    ("IRQ", show_irq),
    ("Timers", show_timers),
    ("I/O port", show_io_port),
    ("Schedule", show_schedule),
    ("GPU", show_gpu),
    ("DMA", show_dma),
    ("GTE", show_gte),
    ("CD-ROM", show_cdrom),
];

