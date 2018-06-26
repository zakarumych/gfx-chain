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
mod queue;
mod submission;

use fnv::FnvHashMap;
use std::collections::hash_map::{
    IntoIter as HashMapIntoIter, Values as HashMapValues, ValuesMut as HashMapValuesMut,
};
use std::ops::{Index, IndexMut};

use hal::queue::QueueFamilyId;

pub use self::family::Family;
pub use self::queue::{Queue, QueueId, QueueIter, QueueIterMut};
pub use self::submission::{Submission, SubmissionId};

/// All schedule on which passes were scheduled.
#[derive(Debug)]
pub struct Schedule<S> {
    map: FnvHashMap<QueueFamilyId, Family<S>>,
}

impl<S> Schedule<S> {
    /// Create new empty `Schedule`
    pub fn new() -> Self {
        Schedule {
            map: FnvHashMap::default(),
        }
    }

    /// The number of families in this schedule.
    pub fn family_count(&self) -> usize {
        self.map.len()
    }

    /// The number of queues in this schedule.
    pub fn queue_count(&self) -> usize {
        self.map.iter().map(|x| x.1.queue_count()).sum()
    }

    /// Iterate over immutable references to families in this schedule.
    pub fn iter(&self) -> HashMapValues<QueueFamilyId, Family<S>> {
        self.map.values()
    }

    /// Iterate over mutable references to families in this schedule
    pub fn iter_mut(&mut self) -> HashMapValuesMut<QueueFamilyId, Family<S>> {
        self.map.values_mut()
    }

    /// Iterate over owned families in this schedule
    pub fn into_iter(self) -> ScheduleIntoIter<S> {
        ScheduleIntoIter(self.map.into_iter())
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

/// Iterator over owned families in this schedule
pub struct ScheduleIntoIter<S>(HashMapIntoIter<QueueFamilyId, Family<S>>);

impl<S> Iterator for ScheduleIntoIter<S> {
    type Item = Family<S>;

    fn next(&mut self) -> Option<Family<S>> {
        self.0.next().map(|x| x.1)
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.0.size_hint()
    }
}

impl<S> IntoIterator for Schedule<S> {
    type Item = Family<S>;
    type IntoIter = ScheduleIntoIter<S>;

    fn into_iter(self) -> ScheduleIntoIter<S> {
        self.into_iter()
    }
}

impl<'a, S> IntoIterator for &'a Schedule<S> {
    type Item = &'a Family<S>;
    type IntoIter = HashMapValues<'a, QueueFamilyId, Family<S>>;

    fn into_iter(self) -> HashMapValues<'a, QueueFamilyId, Family<S>> {
        self.iter()
    }
}

impl<'a, S> IntoIterator for &'a mut Schedule<S> {
    type Item = &'a mut Family<S>;
    type IntoIter = HashMapValuesMut<'a, QueueFamilyId, Family<S>>;

    fn into_iter(self) -> HashMapValuesMut<'a, QueueFamilyId, Family<S>> {
        self.iter_mut()
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
