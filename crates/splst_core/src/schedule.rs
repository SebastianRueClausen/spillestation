//! TODO:
//! * Remove ['Event::IrqTrigger'] and just use ['Event::IrqCheck'] instead. The only difference is
//!   that the CPU triggers an IRQ with ['Event::IrqTrigger'] and then check's if it should handle
//!   the Interrupt. ['Event::IrqCheck'] just forces the CPU to check if any IRQ's are pending. So
//!   a better solution would be just to pass around IRQ state, trigger any IRQ's directly from the
//!   devices and then make the CPU check if there is any IRQ to handle.

use crate::cdrom::CdRomCmd;
use crate::cpu::Irq;
use crate::timer::TimerId;
use crate::Cycle;
use crate::bus::dma::Port;

use std::collections::BinaryHeap;
use std::collections::binary_heap::Iter as BinaryHeapIter;
use std::cmp::Ordering;
use std::fmt;

/// This is reponsible to handling events and timing of the system in general.
pub struct Schedule {
    /// The absolute cycle number, which is the amount of cycles the system has run since startup.
    /// It's used for timing event and allow the devices on ['Bus'] to pick an absolute
    /// cycle to run an event.
    cycle: Cycle,
    /// Event queue. This allows for a fast way to check if any events should run at any given cycle.
    /// Events are sorted in the binary queue such that the next event to run is the root item.
    events: BinaryHeap<EventEntry>,
    /// The cycle when the next event is ready. This is simply an optimization. Just peeking
    /// at the first item in 'events' is constant time, bit requires 4 branches.
    next_event: Cycle,
}

impl Schedule {
    pub fn new() -> Self {
        Self {
            cycle: 0,
            next_event: Cycle::MAX,
            events: BinaryHeap::with_capacity(16),
        }
    }

    fn update_next_event(&mut self) {
       self.next_event = self.events
           .peek()
           .map(|entry| entry.0)
           .unwrap_or(Cycle::MAX);
    }

    /// Returns iter of all event entries in the event heap in arbitary order.
    pub fn iter<'a>(&'a self) -> BinaryHeapIter<'a, EventEntry> {
        self.events.iter()
    }

    /// Schedule an ['Event'] at a given absolute cycle.
    pub fn schedule_at(&mut self, cycle: Cycle, event: Event) {
        self.events.push(EventEntry(cycle, event));
        self.update_next_event();
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
        if self.next_event <= self.cycle {
            let event = self.events.pop().unwrap().1;
            self.update_next_event();
            Some(event) 
        } else {
            None
        }
    }

    pub fn unschedule(&mut self, event: Event) {
        self.events.retain(|entry| {
            entry.1 != event 
        });
        self.update_next_event();
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
    IoPortTransfer,
    IrqCheck,
}

impl fmt::Display for Event {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Event::RunCdRom => {
                write!(f, "Run CDROM")
            }
            Event::RunGpu => {
                write!(f, "Run GPU")
            }
            Event::GpuCmdDone => {
                write!(f, "GPU command done")
            }
            Event::RunDmaChan(port) => {
                write!(f, "Run DMA Channel: {:?}", port)
            }
            Event::CdRomResponse(cmd) => {
                write!(f, "CDROM reponse for command: {}", cmd)
            }
            Event::TimerIrqEnable(id) => {
                write!(f, "Enable IRQ for timer: {}", id)
            }
            Event::RunTimer(id) => {
                write!(f, "Run timer: {}", id)
            }
            Event::IrqTrigger(irq) => {
                write!(f, "Trigger IRQ of type: {}", irq)
            }
            Event::IoPortTransfer => {
                write!(f, "IO Port serial transfer") 
            }
            Event::IrqCheck => {
                write!(f, "Check IRQ status")
            }
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

