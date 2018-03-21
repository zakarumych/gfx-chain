//!
//! This module defines types for execution hierarchy.
//!
//! `Submit` is a piece of work that can be recorded into single primary command buffer.
//! `Submit` contains references to links and semaphores required to wait/signal.
//! `Queue` contains array of `Submit`'s. User is expected to submit corresponding command buffers in the order.
//! `Queue`'s are grouped into `Family`. All queues from one `Family` has identical capabilities.
//! `Schedule` is a set or `Family` instances.
//!

mod family;
mod submit;
mod queue;

use std::collections::hash_map::{HashMap, Values as HashMapValues, ValuesMut as HashMapValuesMut};
use std::ops::{Index, IndexMut};

use hal::queue::QueueFamilyId;

pub use self::family::Family;
pub use self::submit::{Submit, SubmitId};
pub use self::queue::{Queue, QueueId, Submits, SubmitsMut};

/// All schedule on which passes were scheduled.
pub struct Schedule<S> {
    map: HashMap<QueueFamilyId, Family<S>>,
}

impl<S> Schedule<S> {
    /// Iterate over references to all schedule.
    pub fn iter(&self) -> HashMapValues<QueueFamilyId, Family<S>> {
        self.map.values()
    }

    /// Iterate over mutable references to all schedule.
    pub fn iter_mut(&mut self) -> HashMapValuesMut<QueueFamilyId, Family<S>> {
        self.map.values_mut()
    }

    /// Get reference to `Family` instance by the id.
    pub fn get_family(&self, fid: QueueFamilyId) -> Option<&Family<S>> {
        self.map.get(&fid)
    }

    /// Get mutable reference to `Family` instance by the id.
    pub fn get_family_mut(&mut self, fid: QueueFamilyId) -> Option<&mut Family<S>> {
        self.map.get_mut(&fid)
    }

    /// Get mutable reference to `Family` instance by the id.
    /// This function will add empty `Family` if id is not present.
    pub fn ensure_family(&mut self, fid: QueueFamilyId) -> &mut Family<S> {
        self.map.entry(fid).or_insert_with(|| Family::new(fid))
    }

    /// Get reference to `Queue` instance by the id.
    pub fn get_queue(&self, qid: QueueId) -> Option<&Queue<S>> {
        self.get_family(qid.family())
            .and_then(|family| family.get_queue(qid))
    }

    /// Get mutable reference to `Queue` instance by the id.
    pub fn get_queue_mut(&mut self, qid: QueueId) -> Option<&mut Queue<S>> {
        self.get_family_mut(qid.family())
            .and_then(|family| family.get_queue_mut(qid))
    }

    /// Get mutable reference to `Queue` instance by the id.
    /// This function will grow queues array if index is out of bounds.
    pub fn ensure_queue(&mut self, qid: QueueId) -> &mut Queue<S> {
        self.ensure_family(qid.family()).ensure_queue(qid)
    }

    /// Get reference to `Submit` instance by id.
    pub fn get_submit(&self, sid: SubmitId) -> Option<&Submit<S>> {
        self.get_queue(sid.queue())
            .and_then(|queue| queue.get_submit(sid))
    }

    /// Get reference to `Submit` instance by id.
    pub fn get_submit_mut(&mut self, sid: SubmitId) -> Option<&mut Submit<S>> {
        self.get_queue_mut(sid.queue())
            .and_then(|queue| queue.get_submit_mut(sid))
    }
}

impl<S> Default for Schedule<S> {
    fn default() -> Self {
        Schedule {
            map: HashMap::default(),
        }
    }
}

impl<S> Index<QueueFamilyId> for Schedule<S> {
    type Output = Family<S>;

    fn index(&self, fid: QueueFamilyId) -> &Family<S> {
        self.get_family(fid).unwrap()
    }
}

impl<S> IndexMut<QueueFamilyId> for Schedule<S> {
    fn index_mut(&mut self, fid: QueueFamilyId) -> &mut Family<S> {
        self.get_family_mut(fid).unwrap()
    }
}

impl<S> Index<QueueId> for Schedule<S> {
    type Output = Queue<S>;

    fn index(&self, qid: QueueId) -> &Queue<S> {
        self.get_queue(qid).unwrap()
    }
}

impl<S> IndexMut<QueueId> for Schedule<S> {
    fn index_mut(&mut self, qid: QueueId) -> &mut Queue<S> {
        self.get_queue_mut(qid).unwrap()
    }
}

impl<S> Index<SubmitId> for Schedule<S> {
    type Output = Submit<S>;

    fn index(&self, sid: SubmitId) -> &Submit<S> {
        self.get_submit(sid).unwrap()
    }
}

impl<S> IndexMut<SubmitId> for Schedule<S> {
    fn index_mut(&mut self, sid: SubmitId) -> &mut Submit<S> {
        self.get_submit_mut(sid).unwrap()
    }
}
