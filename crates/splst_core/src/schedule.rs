use crate::cdrom::CdRomCmd;
use crate::cpu::Irq;
use crate::timer::TimerId;
use crate::Cycle;
use crate::bus::dma::Port;

use std::collections::BinaryHeap;
use std::collections::binary_heap::Iter as BinaryHeapIter;
use std::cmp::Ordering;
use std::fmt;

#[derive(PartialEq, Eq)]
pub enum Event {
    RunCdRom,
    RunGpu,
    GpuCmdDone,
    RunDmaChan(Port),
    CdRomResponse(CdRomCmd),
    TimerIrqEnable(TimerId),
    RunTimer(TimerId),
    IrqTrigger(Irq),
}

impl fmt::Display for Event {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Event::RunCdRom => write!(f, "Run CDROM"),
            Event::RunGpu => write!(f, "Run GPU"),
            Event::GpuCmdDone => write!(f, "GPU command done"),
            Event::RunDmaChan(port) => write!(f, "Run DMA Channel: {:?}", port),
            Event::CdRomResponse(cmd) => write!(f, "CDROM reponse for command: {}", cmd),
            Event::TimerIrqEnable(id) => write!(f, "Enable IRQ for timer: {}", id),
            Event::RunTimer(id) => write!(f, "Run timer: {}", id),
            Event::IrqTrigger(irq) => write!(f, "Trigger IRQ of type: {}", irq),
        }
    }
}

pub struct EventEntry(pub Cycle, pub Event);

impl PartialEq for EventEntry {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

impl Eq for EventEntry {}

impl PartialOrd for EventEntry {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))  
    }
}

impl Ord for EventEntry {
    /// Sort smallest to largest cycle.
    fn cmp(&self, other: &Self) -> Ordering {
        other.0.cmp(&self.0)
    }
}

/// This is reponsible to handling events and timing of the system in general.
pub struct Schedule {
    /// The absolute cycle number, which is the amount of cycles the system has run since startup.
    /// It's used for timing event and allow the devices on ['Bus'] to pick an absolute
    /// cycle to run an event.
    cycle: Cycle,
    /// Event queue. This allows for a fast way to check if any events should run at any given cycle.
    /// Events are sorted in the binary queue such that the next event to run is the root item.
    events: BinaryHeap<EventEntry>,
}

impl Schedule {
    pub fn new() -> Self {
        Self {
            cycle: 0,
            events: BinaryHeap::with_capacity(16),
        }
    }

    /// Returns iter of all event entries in the event heap in arbitary order.
    pub fn iter<'a>(&'a self) -> BinaryHeapIter<'a, EventEntry> {
        self.events.iter()
    }

    /// Schedule an ['Event'] at a given absolute cycle.
    pub fn schedule_at(&mut self, cycle: Cycle, event: Event) {
        self.events.push(EventEntry(cycle, event));
    }

    /// Schedule an ['Event'] in a given number of cycles.
    pub fn schedule_in(&mut self, cycles: Cycle, event: Event) {
        self.schedule_at(self.cycle + cycles, event);
    }

    /// Schedule an ['Event'] to be executed as soon as possible.
    pub fn schedule_now(&mut self, event: Event) {
        self.schedule_at(0, event);
    }

    /// Returns an event if any is ready.
    pub fn pop_event(&mut self) -> Option<Event> {
        match self.events.peek() {
            Some(entry) if entry.0 <= self.cycle => {
                Some(self.events.pop().unwrap().1)
            }
            _ => None,
        }
    }

    pub fn unschedule(&mut self, event: Event) {
        self.events.retain(|entry| {
            entry.1 != event 
        });
    }

    pub fn cycle(&self) -> Cycle {
        self.cycle
    }

    /// Move a given amount of cycles forward.
    pub fn tick(&mut self, cycles: Cycle) {
        self.cycle += cycles;
    }

    /// Skip to a cycle. It can only skip forward, so if a cycle given is less than the current cycle,
    /// nothing happens.
    pub fn skip_to(&mut self, cycle: Cycle) {
        self.cycle = self.cycle.max(cycle);
    }
}