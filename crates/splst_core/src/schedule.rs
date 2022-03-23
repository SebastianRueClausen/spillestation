//! # TODO
//! - Remove ['Event::IrqTrigger'] and just use ['Event::IrqCheck'] instead. The only difference is
//!   that the CPU triggers an IRQ with ['Event::IrqTrigger'] and then check's if it should handle
//!   the Interrupt. ['Event::IrqCheck'] just forces the CPU to check if any IRQ's are pending. So
//!   a better solution would be just to pass around IRQ state, trigger any IRQ's directly from the
//!   devices and then make the CPU check if there is any IRQ to handle.

use crate::cdrom::CdRomCmd;
use crate::cpu::{Irq, Cpu};
use crate::timer::TimerId;
use crate::SysTime;
use crate::bus::dma::Port;

use std::collections::BinaryHeap;
use std::collections::binary_heap::Iter as BinaryHeapIter;
use std::cmp::Ordering;
use std::fmt;

/// This is reponsible to handling events and timing of the system in general.
pub struct Schedule {
    /// The amount of time the system has been running since startup. It's used for timing event and
    /// allow the devices on ['Bus'] to pick an absolute cycle to run an event.
    now: SysTime,
    /// Event queue. This allows for a fast way to check if any events should run at any given cycle.
    /// Events are sorted in the binary queue such that the next event to run is the root item.
    events: BinaryHeap<EventEntry>,
    /// The time stamp when the next event is ready. This is simply an optimization. Just peeking
    /// at the first item in 'events' is constant time, bit requires 4 branches.
    next_event: SysTime,
}

impl Schedule {
    pub fn new() -> Self {
        Self {
            now: SysTime::ZERO,
            next_event: SysTime::FOREVER,
            events: BinaryHeap::with_capacity(16),
        }
    }

    fn update_next_event(&mut self) {
       self.next_event = self.events
           .peek()
           .map(|entry| entry.0)
           .unwrap_or(SysTime::FOREVER);
    }

    /// Returns iter of all event entries in the event heap in arbitary order.
    pub fn iter<'a>(&'a self) -> BinaryHeapIter<'a, EventEntry> {
        self.events.iter()
    }

    /// Schedule an ['Event'] at a given amount of time since startup.
    pub fn schedule_from_startup(&mut self, time_stamp: SysTime, event: Event) {
        self.events.push(EventEntry(time_stamp, event));
        self.update_next_event();
    }

    /// Schedule an ['Event'] in a given amount of time.
    pub fn schedule_in(&mut self, time: SysTime, event: Event) {
        self.schedule_from_startup(self.now + time, event);
    }

    /// Schedule an ['Event'] to be executed as soon as possible.
    pub fn schedule_now(&mut self, event: Event) {
        self.schedule_from_startup(SysTime::ZERO, event);
    }

    /// Returns an event if any is ready.
    pub fn pop_event(&mut self) -> Option<Event> {
        if self.next_event <= self.now {
            let event = self.events.pop().unwrap().1;
            self.update_next_event();
            Some(event) 
        } else {
            None
        }
    }

    /// Unschedule all events of equal to 'event'.
    pub fn unschedule(&mut self, event: Event) {
        self.events.retain(|entry| entry.1 != event);
        self.update_next_event();
    }

    /// The amount of system time since startup.
    pub fn since_startup(&self) -> SysTime {
        self.now
    }

    /// Advance a given amount of time.
    pub fn advance(&mut self, time: SysTime) {
        self.now = self.now + time;
    }

    /// Skip to an amount of time since startup. It can only skip forward, so if the time given
    /// is less than the current amount of time since startup, then nothing happens.
    pub fn skip_to(&mut self, time: SysTime) {
        self.now = self.now.max(time);
    }
}

#[derive(PartialEq, Eq, Clone)]
pub enum Event {
    // CDROM:
    RunCdRom,
    CdRomSectorDone,
    CdRomResponse(CdRomCmd),

    // GPU:
    RunGpu,
    GpuCmdDone,

    // DMA:
    RunDmaChan(Port),

    // Timers:
    TimerIrqEnable(TimerId),
    RunTimer(TimerId),

    // Interrups:
    IrqTrigger(Irq),
    IrqCheck,

    // IO Ports:
    IoPortTransfer,
}

impl fmt::Display for Event {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Event::RunCdRom => {
                write!(f, "Run CDROM")
            }
            Event::CdRomSectorDone => {
                write!(f, "CDROM sector done")
            }
            Event::CdRomResponse(cmd) => {
                write!(f, "CDROM reponse for command: {}", cmd)
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

pub struct EventEntry(pub SysTime, pub Event);

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

