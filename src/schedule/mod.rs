//!
//! This module defines types for execution hierarchy.
//!
//! `Submission` is a piece of work that can be recorded into single primary command buffer.
//! `Submission` contains references to links and semaphores required to wait/signal.
//! `Queue` contains array of `Submission`'s. User is expected to submission corresponding command buffers in the order.
//! `Queue`'s are grouped into `Family`. All queues from one `Family` has identical capabilities.
//! `Schedule` is a set or `Family` instances.
//!

mod family;
mod submission;
mod queue;

use std::collections::hash_map::{HashMap, Values as HashMapValues, ValuesMut as HashMapValuesMut};
use std::ops::{Index, IndexMut};

use hal::queue::QueueFamilyId;

pub use self::family::Family;
pub use self::submission::{Submission, SubmissionId};
pub use self::queue::{Queue, QueueId, Submissions, SubmissionsMut};

/// All schedule on which passes were scheduled.
pub struct Schedule<S> {
    map: HashMap<QueueFamilyId, Family<S>>,
}

impl<S> Schedule<S> {
    /// Create new empty `Schedule`
    pub fn new() -> Self {
        Schedule {
            map: HashMap::new()
        }
    }

    /// Iterate over references to all schedule.
    pub fn iter(&self) -> HashMapValues<QueueFamilyId, Family<S>> {
        self.map.values()
    }

    /// Iterate over mutable references to all schedule.
    pub fn iter_mut(&mut self) -> HashMapValuesMut<QueueFamilyId, Family<S>> {
        self.map.values_mut()
    }

    /// Get reference to `Family` instance by the id.
    pub fn family(&self, fid: QueueFamilyId) -> Option<&Family<S>> {
        self.map.get(&fid)
    }

    /// Get mutable reference to `Family` instance by the id.
    pub fn family_mut(&mut self, fid: QueueFamilyId) -> Option<&mut Family<S>> {
        self.map.get_mut(&fid)
    }

    /// Get mutable reference to `Family` instance by the id.
    /// This function will add empty `Family` if id is not present.
    pub fn ensure_family(&mut self, fid: QueueFamilyId) -> &mut Family<S> {
        self.map.entry(fid).or_insert_with(|| Family::new(fid))
    }

    /// Get reference to `Queue` instance by the id.
    pub fn queue(&self, qid: QueueId) -> Option<&Queue<S>> {
        self.family(qid.family())
            .and_then(|family| family.queue(qid))
    }

    /// Get mutable reference to `Queue` instance by the id.
    pub fn queue_mut(&mut self, qid: QueueId) -> Option<&mut Queue<S>> {
        self.family_mut(qid.family())
            .and_then(|family| family.queue_mut(qid))
    }

    /// Get mutable reference to `Queue` instance by the id.
    /// This function will grow queues array if index is out of bounds.
    pub fn ensure_queue(&mut self, qid: QueueId) -> &mut Queue<S> {
        self.ensure_family(qid.family()).ensure_queue(qid)
    }

    /// Get reference to `Submission` instance by id.
    pub fn submission(&self, sid: SubmissionId) -> Option<&Submission<S>> {
        self.queue(sid.queue())
            .and_then(|queue| queue.submission(sid))
    }

    /// Get reference to `Submission` instance by id.
    pub fn submission_mut(&mut self, sid: SubmissionId) -> Option<&mut Submission<S>> {
        self.queue_mut(sid.queue())
            .and_then(|queue| queue.submission_mut(sid))
    }
}

impl<S> Default for Schedule<S> {
    fn default() -> Self {
        Schedule::new()
    }
}

impl<S> Index<QueueFamilyId> for Schedule<S> {
    type Output = Family<S>;

    fn index(&self, fid: QueueFamilyId) -> &Family<S> {
        self.family(fid).unwrap()
    }
}

impl<S> IndexMut<QueueFamilyId> for Schedule<S> {
    fn index_mut(&mut self, fid: QueueFamilyId) -> &mut Family<S> {
        self.family_mut(fid).unwrap()
    }
}

impl<S> Index<QueueId> for Schedule<S> {
    type Output = Queue<S>;

    fn index(&self, qid: QueueId) -> &Queue<S> {
        self.queue(qid).unwrap()
    }
}

impl<S> IndexMut<QueueId> for Schedule<S> {
    fn index_mut(&mut self, qid: QueueId) -> &mut Queue<S> {
        self.queue_mut(qid).unwrap()
    }
}

impl<S> Index<SubmissionId> for Schedule<S> {
    type Output = Submission<S>;

    fn index(&self, sid: SubmissionId) -> &Submission<S> {
        self.submission(sid).unwrap()
    }
}

impl<S> IndexMut<SubmissionId> for Schedule<S> {
    fn index_mut(&mut self, sid: SubmissionId) -> &mut Submission<S> {
        self.submission_mut(sid).unwrap()
    }
}
