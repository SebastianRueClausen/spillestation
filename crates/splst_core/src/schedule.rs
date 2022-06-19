use crate::cpu::Irq;
use crate::bus::Bus;
use crate::spu::Spu;
use crate::gpu::Gpu;
use crate::timer::{Timers, TimerId};
use crate::cdrom::CdRom;
use crate::io_port::IoPort;
use crate::{SysTime, Timestamp};
use crate::bus::dma;

use std::collections::BinaryHeap;
use std::cmp::Ordering;
use std::fmt;

/// This is reponsible to handling events and timing of the system in general.
pub struct Schedule {
    /// The ID of the next event scheduled.
    next_event_id: EventId,
    /// The amount of time since startup.
    now: Timestamp,
    /// Priority queue of pending events.
    events: BinaryHeap<EventEntry>,
    /// The timestamp when the next event is ready. This is simply an optimization. Just peeking
    /// at the first item in `events` is constant time, but requires 4 branches.
    next_event: Timestamp,
}

impl Schedule {
    pub fn new() -> Self {
        Self {
            next_event_id: EventId(0),
            now: Timestamp::STARTUP,
            next_event: Timestamp::NEVER,
            events: BinaryHeap::with_capacity(16),
        }
    }

    /// Get a new unique event ID.
    fn get_event_id(&mut self) -> EventId {
        let id = self.next_event_id;
        self.next_event_id = EventId(self.next_event_id.0 + 1);
        id
    }

    /// Update the `next_event` field.
    fn update_next_event(&mut self) {
       self.next_event = self.events
           .peek()
           .map(|entry| entry.ready)
           .unwrap_or(Timestamp::NEVER);
    }

    fn push_event(&mut self, entry: EventEntry) {
        self.events.push(entry);
        self.update_next_event();
    }

    /// Returns iter of all event entries in the event heap in arbitary order.
    pub fn iter_event_entries<'a>(&'a self) -> impl Iterator<Item = &'a EventEntry> {
        self.events.iter()
    }

    /// Schedule an [`Event`] to trigger in 'time' and repeat with an interval of `time`.
    pub(crate) fn schedule_repeat(&mut self, time: SysTime, event: Event) -> EventId {
        let id = self.get_event_id();
        let ready = self.now() + time;
        let entry = EventEntry {
            mode: RepeatMode::Repeat(time),
            event,
            id,
            ready,
        };
        self.push_event(entry);
        id
    }

    /// Schedule an [`Event`] to trigger in `time` and don't repeat it.
    pub(crate) fn schedule(&mut self, time: SysTime, event: Event) -> EventId {
        let id = self.get_event_id();
        let ready = self.now() + time;
        let entry = EventEntry {
            event, id, ready, mode: RepeatMode::Once,
        };
        self.push_event(entry);
        id
    }

    /// Trigger an [`Event`] now and repeat with an interval of `time`.
    #[allow(dead_code)]
    pub(crate) fn trigger_repeat(&mut self, time: SysTime, event: Event) -> EventId {
        let id = self.get_event_id();
        let entry = EventEntry {
            ready: Timestamp::STARTUP,
            mode: RepeatMode::Repeat(time),
            event,
            id,
        };

        self.events.push(entry);
        self.next_event = Timestamp::STARTUP;

        id
    }

    /// Trigger an [`Event`] as soon as possible and don't repeat it.
    pub(crate) fn trigger(&mut self, event: Event) -> EventId {
        let id = self.get_event_id();
        let entry = EventEntry {
            event, id, ready: Timestamp::STARTUP, mode: RepeatMode::Once,
        };

        self.events.push(entry);
        self.next_event = Timestamp::STARTUP;

        id
    }

    /// Get a pending event if there is any. Returns the action and name.
    pub(crate) fn get_pending_event(&mut self) -> Option<Event> {
        if self.next_event <= self.now {
            // Since `self.next_event` is set to `SysTime::FOREVER` if there aren't any pending
            // events, it should be safe to assume that the heap isn't empty. The only way that
            // could be false, is if 'self.now' is about to overflow, which isn't allowed to
            // happend anyway.
            let mut entry = self.events.pop().unwrap();
            let event = entry.event;

            // Added the event again if it's in repeat mode.
            if let RepeatMode::Repeat(time) = entry.mode {
                entry.ready = entry.ready + time;
                self.events.push(entry);
            }

            self.update_next_event();
            Some(event) 
        } else {
            None
        }
    }

    /// Trigger a scheduled [`Event`] early.
    #[allow(dead_code)]
    pub(crate) fn trigger_early(&mut self, id: EventId) {
        if let Some(entry) = self.events.iter().find(|e| e.id == id) {
            let mut entry = entry.clone();
            
            // Unschedule the the existing event and trigger it immediately.
            self.unschedule(id);
            self.trigger(entry.event);
            
            // Check if it should be repeated.
            if let RepeatMode::Repeat(time) = entry.mode {
                entry.ready = self.now() + time;
                self.events.push(entry.clone());
            }

            // Triggering the event already updates `next_event` so we don't have to update it
            // here.
        } else {
            warn!("triggering non-existing event early");
        }
    }
    
    /// Make event with `id` repeat at an interval of `time`. It doesn't change when the event
    /// will trigger next. If no active event exists with `id`, nothing will happend.
    pub(crate) fn repeat_every(&mut self, time: SysTime, id: EventId) {
        let mut entry = None;

        // This is a bit wasteful, but i don't see a better way and we rarely do this anyway so
        // it doesn't really matter.
        self.events.retain(|event| {
            if event.id == id {
                entry = Some(event.clone());
                false
            } else {
                true
            }
        });

        let Some(mut entry) = entry else {
            return;
        };

        entry.mode = RepeatMode::Repeat(time);
        self.events.push(entry.clone());
    
        // Since this doesn't change when the next event will trigger, we don't have to update
        // `next_event`.
    }
    
    /// Unschedule an [`Event`].
    pub(crate) fn unschedule(&mut self, id: EventId) {
        self.events.retain(|event| event.id != id);
        self.update_next_event();
    }
    
    /// Get the amount of time until event with `id` is ready to trigger. It will return 'None'
    /// if no event exists with the id `id`.
    pub(crate) fn time_until_event(&self, id: EventId) -> Option<SysTime> {
        self.iter_event_entries()
            .find(|entry| entry.id == id)
            .map(|entry| {
                entry.ready
                    .time_since_startup()
                    .saturating_sub(self.now().time_since_startup())
            })
    }

    /// The amount of system time since startup.
    pub fn now(&self) -> Timestamp {
        self.now
    }

    /// Advance a given amount of time.
    pub(crate) fn advance(&mut self, time: SysTime) {
        self.now = self.now + time;
    }

    /// Skip to an amount of time since startup. It can only skip forward, so if the time given
    /// is less than the current amount of time since startup, then nothing happens.
    pub(crate) fn skip_to(&mut self, time: Timestamp) {
        self.now = self.now.max(time);
    }
}

/// A unique ID for each event. This can be used to modify, cancel or run the event early.
///
/// If the event type is [`RepeatMode::Once`], the ID is only valid until the event is triggered.
/// If the event type is 'Repeat', the ID is valid until it's cancelled.
#[derive(Clone, Copy, PartialEq)]
pub struct EventId(u64);

#[derive(Clone, Copy)]
pub enum RepeatMode {
    ///  The event is removed from the event queue once it's triggered.
    Once,
    /// The event triggers continuously at a given interval until it's stopped.
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
    /// Almost the same as [`Irq`], but it simply forces the CPU to check if there are any pending
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
    Spu(fn(&mut Spu, &mut Schedule, &mut CdRom)),
    /// Stop CPU execution and return from [`crate::cpu::Cpu::run`].
    ExecutionTimeout,
}

impl fmt::Display for Event {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Event::IrqCheck => f.write_str("interrupt check"),
            Event::Irq(irq) => write!(f, "interrupt of type {irq}"),
            Event::Dma(port, _) => write!(f, "DMA port {port}"),
            Event::Gpu(..) => f.write_str("GPU"),
            Event::CdRom(..) => f.write_str("CD-ROM"),
            Event::Timer(..) => f.write_str("timer"),
            Event::IoPort(..) => f.write_str("I/O Port"),
            Event::Spu(..) => f.write_str("SPU"),
            Event::ExecutionTimeout => f.write_str("execution timeout"),
        }
    }
}

#[derive(Clone)]
pub struct EventEntry {
    /// The type and data of the event.
    pub event: Event,
    /// When the event is ready to trigger. It may not be triggered at this exact time, but not
    /// before.
    pub ready: Timestamp,
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
