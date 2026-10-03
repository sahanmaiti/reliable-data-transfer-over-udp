// CS-30003: Reliable Data Transfer over UDP
// Author: Kashish Gupta
// Component: Retransmission Timing & RTO - Non-blocking Retransmission Timers

use std::collections::HashMap;
use std::hash::Hash;
use std::time::{Duration, Instant};

/// Generation identifier used to prevent stale timer expiry events from triggering.
pub type TimerId = u64;

/// A non-blocking, generation-safe retransmission timer.
///
/// Designed to eliminate race conditions between asynchronous socket operations
/// and timer expirations: whenever the timer is cancelled or re-armed, its internal
/// `generation` counter increments. Any timeout callback referencing an older generation
/// is rejected as stale.
#[derive(Debug, Clone)]
pub struct RetransmissionTimer {
    /// Scheduled expiry instant, or None if inactive.
    deadline: Option<Instant>,
    /// Monotonically increasing generation ID.
    generation: TimerId,
}

impl RetransmissionTimer {
    /// Creates a new inactive retransmission timer.
    pub fn new() -> Self {
        Self {
            deadline: None,
            generation: 0,
        }
    }

    /// Arms or restarts the timer for the specified duration.
    /// Returns the new active `TimerId` (generation).
    pub fn start(&mut self, duration: Duration) -> TimerId {
        self.start_at(Instant::now(), duration)
    }

    /// Arms the timer with an explicit reference instant (useful for deterministic tests).
    pub fn start_at(&mut self, now: Instant, duration: Duration) -> TimerId {
        self.generation = self.generation.wrapping_add(1);
        self.deadline = Some(now + duration);
        self.generation
    }

    /// Cancels the active timer. Increments generation so in-flight events are invalidated.
    pub fn cancel(&mut self) -> TimerId {
        self.deadline = None;
        self.generation = self.generation.wrapping_add(1);
        self.generation
    }

    /// Checks if the timer is currently running.
    pub fn is_active(&self) -> bool {
        self.deadline.is_some()
    }

    /// Returns the current active generation ID.
    pub fn generation(&self) -> TimerId {
        self.generation
    }

    /// Returns the scheduled deadline instant, if active.
    pub fn deadline(&self) -> Option<Instant> {
        self.deadline
    }

    /// Returns the duration remaining until the timer expires.
    pub fn remaining(&self) -> Option<Duration> {
        self.remaining_at(Instant::now())
    }

    /// Returns remaining duration relative to an explicit instant.
    pub fn remaining_at(&self, now: Instant) -> Option<Duration> {
        self.deadline.map(|dl| {
            if now >= dl {
                Duration::ZERO
            } else {
                dl - now
            }
        })
    }

    /// Returns `true` if the timer is active and current wall clock has passed deadline.
    pub fn is_expired(&self) -> bool {
        self.is_expired_at(Instant::now())
    }

    /// Returns `true` if the timer is active and has expired relative to `now`.
    pub fn is_expired_at(&self, now: Instant) -> bool {
        match self.deadline {
            Some(dl) => now >= dl,
            None => false,
        }
    }

    /// Checks whether the timer expired for a specific armed generation ID.
    /// If `expected_generation != self.generation`, the event is considered stale.
    pub fn is_expired_for(&self, expected_generation: TimerId) -> bool {
        self.is_expired_for_at(expected_generation, Instant::now())
    }

    /// Checks expiration for a specific generation relative to `now`.
    pub fn is_expired_for_at(&self, expected_generation: TimerId, now: Instant) -> bool {
        if self.generation != expected_generation {
            return false; // Stale timer event!
        }
        self.is_expired_at(now)
    }
}

impl Default for RetransmissionTimer {
    fn default() -> Self {
        Self::new()
    }
}

/// An entry in the MultiTimer queue.
#[derive(Debug, Clone)]
struct MultiTimerEntry {
    deadline: Instant,
    generation: TimerId,
}

/// Manages multiple independent packet timers (essential for Selective Repeat).
#[derive(Debug, Clone)]
pub struct MultiTimer<K: Hash + Eq + Clone> {
    timers: HashMap<K, MultiTimerEntry>,
    global_generation: TimerId,
}

impl<K: Hash + Eq + Clone> MultiTimer<K> {
    /// Creates a new empty multi-timer collection.
    pub fn new() -> Self {
        Self {
            timers: HashMap::new(),
            global_generation: 0,
        }
    }

    /// Sets or restarts an individual timer for key `K`.
    /// Returns the armed `TimerId`.
    pub fn start_timer(&mut self, key: K, duration: Duration) -> TimerId {
        self.start_timer_at(key, Instant::now(), duration)
    }

    /// Sets an individual timer relative to an explicit instant.
    pub fn start_timer_at(&mut self, key: K, now: Instant, duration: Duration) -> TimerId {
        self.global_generation = self.global_generation.wrapping_add(1);
        let gen = self.global_generation;
        self.timers.insert(
            key,
            MultiTimerEntry {
                deadline: now + duration,
                generation: gen,
            },
        );
        gen
    }

    /// Cancels the timer for key `K`. Returns true if a timer existed.
    pub fn cancel_timer(&mut self, key: &K) -> bool {
        self.timers.remove(key).is_some()
    }

    /// Checks if a timer exists and has expired.
    pub fn is_expired(&self, key: &K) -> bool {
        self.is_expired_at(key, Instant::now())
    }

    /// Checks if key `K` is expired relative to `now`.
    pub fn is_expired_at(&self, key: &K, now: Instant) -> bool {
        match self.timers.get(key) {
            Some(entry) => now >= entry.deadline,
            None => false,
        }
    }

    /// Drains and returns all keys whose deadlines have expired at `now`.
    pub fn pop_expired_at(&mut self, now: Instant) -> Vec<K> {
        let expired_keys: Vec<K> = self
            .timers
            .iter()
            .filter(|(_, entry)| now >= entry.deadline)
            .map(|(k, _)| k.clone())
            .collect();

        for k in &expired_keys {
            self.timers.remove(k);
        }

        expired_keys
    }

    /// Finds the earliest scheduled deadline across all active timers.
    pub fn earliest_deadline(&self) -> Option<Instant> {
        self.timers.values().map(|e| e.deadline).min()
    }

    /// Number of active timers.
    pub fn len(&self) -> usize {
        self.timers.len()
    }

    /// Returns true if no timers are active.
    pub fn is_empty(&self) -> bool {
        self.timers.is_empty()
    }

    /// Clears all timers.
    pub fn clear(&mut self) {
        self.timers.clear();
    }
}

impl<K: Hash + Eq + Clone> Default for MultiTimer<K> {
    fn default() -> Self {
        Self::new()
    }
}
