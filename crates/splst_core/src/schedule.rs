use crate::cpu::{Irq, Cpu};
use crate::bus::Bus;
use crate::gpu::Gpu;
use crate::timer::{Timers, TimerId};
use crate::cdrom::CdRom;
use crate::io_port::IoPort;
use crate::SysTime;
use crate::bus::dma;

use std::collections::BinaryHeap;
use std::collections::binary_heap::Iter as BinaryHeapIter;
use std::cmp::Ordering;
use std::fmt;

/// This is reponsible to handling events and timing of the system in general.
pub struct Schedule {
    /// The ID of the next event scheduled.
    next_event_id: EventId,
    /// The amount of time the system has been running since startup. It's used for timing event and
    /// allow the devices on ['Bus'] to pick an absolute cycle to run an event.
    now: SysTime,
    /// Event queue. This allows for a fast way to check if any events should run at any given cycle.
    /// Events are sorted in the binary queue such that the next event to run is the root item.
    events: BinaryHeap<EventEntry>,
    /// The timestamp when the next event is ready. This is simply an optimization. Just peeking
    /// at the first item in 'events' is constant time, bit requires 4 branches.
    next_event: SysTime,
}

impl Schedule {
    pub fn new() -> Self {
        Self {
            next_event_id: EventId(0),
            now: SysTime::ZERO,
            next_event: SysTime::FOREVER,
            events: BinaryHeap::with_capacity(16),
        }
    }

    /// Get a new unique event ID.
    fn get_event_id(&mut self) -> EventId {
        let id = self.next_event_id;
        self.next_event_id = EventId(self.next_event_id.0 + 1);
        id
    }

    /// Update the 'next_event' field.
    fn update_next_event(&mut self) {
       self.next_event = self.events
           .peek()
           .map(|entry| entry.ready)
           .unwrap_or(SysTime::FOREVER);
    }

    fn push_event(&mut self, entry: EventEntry) {
        self.events.push(entry);
        self.update_next_event();
    }

    /// Returns iter of all event entries in the event heap in arbitary order.
    pub fn iter_event_entries<'a>(&'a self) -> BinaryHeapIter<'a, EventEntry> {
        self.events.iter()
    }

    /// Schedule an ['Event'] to trigger in 'time' and repeat with an interval of 'time'.
    pub fn schedule_repeat(&mut self, time: SysTime, event: Event) -> EventId {
        let id = self.get_event_id();
        let ready = self.since_startup() + time;
        let entry = EventEntry {
            event, id, ready, mode: RepeatMode::Repeat(time),
        };
        self.push_event(entry);
        id
    }

    /// Schedule an ['Event'] to trigger in 'time' and don't repeat it.
    pub fn schedule(&mut self, time: SysTime, event: Event) -> EventId {
        let id = self.get_event_id();
        let ready = self.since_startup() + time;
        let entry = EventEntry {
            event, id, ready, mode: RepeatMode::Once,
        };
        self.push_event(entry);
        id
    }

    /// Trigger an ['Event'] now and repeat with an interval of 'time'.
    pub fn trigger_repeat(&mut self, time: SysTime, event: Event) -> EventId {
        let id = self.get_event_id();
        let entry = EventEntry {
            event, id, ready: SysTime::ZERO, mode: RepeatMode::Repeat(time),
        };

        // We already know that the next event is ready immediatly
        self.events.push(entry);
        self.next_event = SysTime::ZERO;

        id
    }

    /// Trigger an ['Event'] as soon as possible and don't repeat it.
    pub fn trigger(&mut self, event: Event) -> EventId {
        let id = self.get_event_id();
        let entry = EventEntry {
            event, id, ready: SysTime::ZERO, mode: RepeatMode::Once,
        };

        // We already know that the next event is ready immediatly
        self.events.push(entry);
        self.next_event = SysTime::ZERO;

        id
    }

    /// Get a pending event if there is any. Returns the action and name.
    pub fn get_pending_event(&mut self) -> Option<Event> {
        if self.next_event <= self.now {
            // Since 'self.next_event' is set to 'SysTime::FOREVER' if there aren't any pending
            // events, it should be safe to assume that the heap isn't empty. The only way that
            // could be false, is if 'self.now' is about to overflow, which isn't allowed to
            // happend anyway.
            let mut entry = self.events.pop().unwrap();
            let event = entry.event;

            // Added the event again if it's in repeat mode.
            if let RepeatMode::Repeat(time) = entry.mode {
                entry.ready = self.since_startup() + time;
                self.events.push(entry);
            }

            self.update_next_event();
            Some(event) 
        } else {
            None
        }
    }

    /// Trigger a scheduled ['Event'] early.
    pub fn trigger_early(&mut self, id: EventId) {
        if let Some(entry) = self.events.iter().find(|e| e.id == id) {
            let mut entry = entry.clone();
            
            // Unschedule the the existing event and trigger it immediately.
            self.unschedule(id);
            self.trigger(entry.event);
            
            // Check if it should be repeated.
            if let RepeatMode::Repeat(time) = entry.mode {
                entry.ready = self.since_startup() + time;
                self.events.push(entry.clone());
            }

            // Triggering the event already updates 'next_event' so we don't have to update it
            // here.
        } else {
            warn!("triggering non-existing event early");
        }
    }

    /// Unschedule an ['Event'].
    pub fn unschedule(&mut self, id: EventId) {
        self.events.retain(|event| event.id != id);
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

/// A unique ID for each event. This can be used to modify, cancel or run the event early.
///
/// If the event type is 'Once', the ID is only valid until the event is triggered. If the event
/// type is 'Repeat', the ID is valid until it's cancelled.
#[derive(Clone, Copy, PartialEq)]
pub struct EventId(u64);

#[derive(Clone, Copy)]
pub enum RepeatMode {
    /// 'Once' means that the event is removed from the event queue once it's triggered.
    Once,
    /// 'Repeat' means that the event triggers continuously at a given interval until it's stopped.
    Repeat(SysTime),
}

impl fmt::Display for RepeatMode {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            RepeatMode::Once => f.write_str("once"),
            RepeatMode::Repeat(interval) => {
                write!(f, "every {} cycles", interval.as_cpu_cycles())
            }
        }
    }
}

/// The type of the event and associated data.
#[derive(Clone, Copy)]
pub enum Event {
    /// Trigger a hardware interrupt.
    Irq(Irq),
    /// Almost the same as ['Irq'], but it simply forces the CPU to check if there are any pending
    /// interrupts to handle, but doesn't trigger any new interrupts.
    IrqCheck,
    /// A DMA event. For instance executing a transfer to or from a port.
    Dma(dma::Port, fn(&mut Bus, dma::Port)),
    /// A GPU event. For instance running the GPU for a period of time or marking the end of a
    /// draw command. Since the GPU can cause timer synchronisation, the callback takes a reference
    /// to the system timers.
    Gpu(fn(&mut Gpu, &mut Schedule, &mut Timers)),
    /// Either running the CDROM drive or triggering an asynchronous response.
    CdRom(fn(&mut CdRom, &mut Schedule)),
    /// Updating the a specific timer.
    Timer(TimerId, fn(&mut Timers, &mut Schedule, TimerId)),
    IoPort(fn(&mut IoPort, &mut Schedule)),
}

impl fmt::Display for Event {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Event::IrqCheck => f.write_str("interrupt check"),
            Event::Irq(irq) => write!(f, "interrupt of type {irq}"),
            Event::Dma(port, ..) => write!(f, "DMA port {port}"),
            Event::Gpu(..) => f.write_str("GPU"),
            Event::CdRom(..) => f.write_str("CD-ROM"),
            Event::Timer(..) => f.write_str("timer"),
            Event::IoPort(..) => f.write_str("I/O Port"),
        }
    }
}

#[derive(Clone)]
pub struct EventEntry {
    /// The type and data of the event.
    pub event: Event,
    /// The amount of system time since startup that the event is ready to get triggered. This may
    /// be lower than the current time since startup, in which case the event will get triggered as
    /// soon as possible.
    pub ready: SysTime,
    /// Which repeat mode the event has.
    pub mode: RepeatMode,
    /// The ID of the event.
    id: EventId,
}

impl EventEntry {
    pub fn id(&self) -> EventId {
        self.id
    }
}

impl PartialEq for EventEntry {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl Eq for EventEntry {}

/// Order largest to smallest.
impl PartialOrd for EventEntry {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(other.ready.cmp(&self.ready))
    }
}

/// Order largest to smallest.
impl Ord for EventEntry {
    fn cmp(&self, other: &Self) -> Ordering {
        other.ready.cmp(&self.ready)
    }
}

pub fn trigger_event(cpu: &mut Cpu, event: Event) {
    match event {
        Event::IrqCheck => cpu.check_for_pending_irq(),
        Event::Dma(port, callback) => callback(&mut cpu.bus, port),
        Event::CdRom(callback) => callback(&mut cpu.bus.cdrom, &mut cpu.bus.schedule),
        Event::IoPort(callback) => callback(&mut cpu.bus.io_port, &mut cpu.bus.schedule),
        Event::Timer(id, callback) => callback(&mut cpu.bus.timers, &mut cpu.bus.schedule, id),
        Event::Gpu(callback) => {
            callback(&mut cpu.bus.gpu, &mut cpu.bus.schedule, &mut cpu.bus.timers);
        }
        Event::Irq(irq) => {
            cpu.bus.irq_state.trigger(irq);
            cpu.check_for_pending_irq();
        }
    }
}
