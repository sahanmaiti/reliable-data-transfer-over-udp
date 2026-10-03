use std::cmp::{Ordering, Reverse};
use std::collections::BinaryHeap;
use std::time::{Duration, Instant};

use super::Delivery;

/// One packet waiting in the queue.
struct Scheduled {
    deliver_at: Instant, // when it may be handed to the receiver
    order: u64,          // insertion number: breaks ties deterministically
    data: Vec<u8>,       // raw wire bytes
}

// The heap needs a total order: earliest deliver_at first,
// then earliest insertion number.
impl PartialEq for Scheduled {
    fn eq(&self, other: &Self) -> bool {
        self.deliver_at == other.deliver_at && self.order == other.order
    }
}
impl Eq for Scheduled {}
impl PartialOrd for Scheduled {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for Scheduled {
    fn cmp(&self, other: &Self) -> Ordering {
        self.deliver_at
            .cmp(&other.deliver_at)
            .then(self.order.cmp(&other.order))
    }
}

/// Holds packets until their delivery time. This is what turns the
/// emulator's `delay_ms` values into REAL reordering: a packet with a
/// bigger delay simply leaves the queue later than packets sent after it.
pub struct ChannelScheduler {
    // BinaryHeap pops the LARGEST item, so Reverse makes it pop the EARLIEST.
    heap: BinaryHeap<Reverse<Scheduled>>,
    next_order: u64,
}

impl ChannelScheduler {
    pub fn new() -> Self {
        ChannelScheduler {
            heap: BinaryHeap::new(),
            next_order: 0,
        }
    }

    /// Queue the output of `Channel::process`, using the real clock.
    pub fn schedule(&mut self, deliveries: Vec<Delivery>) {
        self.schedule_at(Instant::now(), deliveries);
    }

    /// Same, but with an explicit "now" so tests don't need to sleep.
    pub fn schedule_at(&mut self, now: Instant, deliveries: Vec<Delivery>) {
        for d in deliveries {
            self.heap.push(Reverse(Scheduled {
                deliver_at: now + Duration::from_millis(d.delay_ms),
                order: self.next_order,
                data: d.data,
            }));
            self.next_order += 1;
        }
    }

    /// Returns one packet whose delivery time has arrived, or None.
    /// Call in a loop: `while let Some(p) = scheduler.pop_ready() { ... }`
    pub fn pop_ready(&mut self) -> Option<Vec<u8>> {
        self.pop_ready_at(Instant::now())
    }

    /// Same, but with an explicit "now" (used by the tests).
    pub fn pop_ready_at(&mut self, now: Instant) -> Option<Vec<u8>> {
        let ready = matches!(self.heap.peek(), Some(Reverse(top)) if top.deliver_at <= now);
        if ready {
            self.heap.pop().map(|Reverse(s)| s.data)
        } else {
            None
        }
    }

    /// When the next packet becomes ready (None if the queue is empty).
    /// The socket loop can use this as its read timeout.
    pub fn next_deadline(&self) -> Option<Instant> {
        self.heap.peek().map(|Reverse(s)| s.deliver_at)
    }

    pub fn len(&self) -> usize {
        self.heap.len()
    }

    pub fn is_empty(&self) -> bool {
        self.heap.is_empty()
    }
}

impl Default for ChannelScheduler {
    fn default() -> Self {
        Self::new()
    }
}
#[cfg(test)]
mod tests {
    use super::super::{Channel, ChannelConfig};
    use super::*;

    fn d(delay_ms: u64, tag: u8) -> Delivery {
        Delivery {
            delay_ms,
            data: vec![tag],
        }
    }

    #[test]
    fn releases_in_delivery_time_order() {
        let t0 = Instant::now();
        let mut s = ChannelScheduler::new();
        s.schedule_at(t0, vec![d(30, 1), d(10, 2), d(20, 3)]);
        let late = t0 + Duration::from_millis(100);
        let mut got = Vec::new();
        while let Some(p) = s.pop_ready_at(late) {
            got.push(p[0]);
        }
        assert_eq!(got, vec![2, 3, 1]); // 10 ms, 20 ms, 30 ms
    }

    #[test]
    fn nothing_is_ready_before_its_deadline() {
        let t0 = Instant::now();
        let mut s = ChannelScheduler::new();
        s.schedule_at(t0, vec![d(50, 1)]);
        assert!(s.pop_ready_at(t0 + Duration::from_millis(49)).is_none());
        assert_eq!(s.len(), 1);
        assert!(s.pop_ready_at(t0 + Duration::from_millis(50)).is_some());
        assert!(s.is_empty());
    }

    #[test]
    fn equal_deadlines_keep_arrival_order() {
        let t0 = Instant::now();
        let mut s = ChannelScheduler::new();
        s.schedule_at(t0, vec![d(20, 1), d(20, 2), d(20, 3)]);
        let late = t0 + Duration::from_millis(20);
        let mut got = Vec::new();
        while let Some(p) = s.pop_ready_at(late) {
            got.push(p[0]);
        }
        assert_eq!(got, vec![1, 2, 3]);
    }

    #[test]
    fn next_deadline_is_the_earliest() {
        let t0 = Instant::now();
        let mut s = ChannelScheduler::new();
        assert!(s.next_deadline().is_none());
        s.schedule_at(t0, vec![d(40, 1), d(15, 2)]);
        assert_eq!(s.next_deadline(), Some(t0 + Duration::from_millis(15)));
    }

    /// Sends n packets (one every 10 ms) through Channel + Scheduler and
    /// counts how many arrive after a higher sequence number.
    fn count_reordered(reorder: f64, n: u32) -> u32 {
        let config = ChannelConfig {
            seed: 42,
            loss: 0.0,
            duplicate: 0.0,
            reorder,
            reorder_extra_ms: 50,
            corrupt: 0.0,
            base_delay_ms: 50,
            jitter_ms: 0, // so ONLY reordering can change the order
        };
        let mut ch = Channel::new(config);
        let mut s = ChannelScheduler::new();
        let t0 = Instant::now();
        for i in 0..n {
            let now = t0 + Duration::from_millis(i as u64 * 10);
            s.schedule_at(now, ch.process(&i.to_be_bytes()));
        }
        let end = t0 + Duration::from_secs(3600);
        let mut max_seen = 0u32;
        let mut reordered = 0;
        while let Some(p) = s.pop_ready_at(end) {
            let seq = u32::from_be_bytes(p[..4].try_into().unwrap());
            if seq < max_seen {
                reordered += 1;
            } else {
                max_seen = seq;
            }
        }
        reordered
    }

    #[test]
    fn reorder_zero_keeps_perfect_order() {
        assert_eq!(count_reordered(0.0, 2000), 0);
    }

    #[test]
    fn reorder_fifteen_percent_really_reorders_packets() {
        // This proves reordering happens at the RECEIVER, not just in stats.
        let r = count_reordered(0.15, 2000);
        assert!(r > 200 && r < 400, "reordered {}", r);
    }
}