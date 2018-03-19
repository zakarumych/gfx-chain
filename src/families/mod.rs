//!
//! This module defines types for execution hierarchy.
//!
//! `Submit` is a piece of work that can be recorded into single primary command buffer.
//! `Submit` contains references to links and semaphores required to wait/signal.
//! `Queue` contains array of `Submit`'s. User is expected to submit corresponding command buffers in the order.
//! `Queue`'s are grouped into `Family`. All queues from one `Family` has identical capabilities.
//! `Families` is a set or `Family` instances.
//!

mod queue;

use std::collections::hash_map::{HashMap, Iter as HashMapIter};
use std::ops::{Index, IndexMut};

use hal::queue::QueueFamilyId;

use resource::{Id, Buffer, Image};

pub use self::queue::{Queue, QueueId, Submit, SubmitId};

/// Instances of this type contains array of `Queue`s.
/// All contained queues has identical capabilities.
#[derive(Clone, Debug)]
pub struct Family {
    id: QueueFamilyId,
    queues: Vec<Queue>,
}

impl Family {
    /// Create new empty `Family`
    pub fn new(id: QueueFamilyId) -> Self {
        Family {
            id,
            queues: Vec::default(),
        }
    }

    /// Get reference to `Queue` instance by the id.
    ///
    /// # Panic
    ///
    /// This function will panic if requested queue isn't part of this family.
    ///
    pub fn get_queue(&self, qid: QueueId) -> Option<&Queue> {
        assert_eq!(self.id, qid.family());
        self.queues.get(qid.index())
    }

    /// Get mutable reference to `Queue` instance by the id.
    ///
    /// # Panic
    ///
    /// This function will panic if requested queue isn't part of this family.
    ///
    pub fn get_queue_mut(&mut self, qid: QueueId) -> Option<&mut Queue> {
        assert_eq!(self.id, qid.family());
        self.queues.get_mut(qid.index())
    }

    /// Get mutable reference to `Queue` instance by the id.
    /// This function will grow queues array if index is out of bounds.
    ///
    /// # Panic
    ///
    /// This function will panic if requested queue isn't part of this family.
    ///
    pub fn ensure_queue(&mut self, qid: QueueId) -> &mut Queue {
        assert_eq!(self.id, qid.family());
        let len = self.queues.len();
        self.queues
            .extend((len..qid.index() + 1).map(|_| Queue::new(qid)));
        &mut self.queues[qid.index()]
    }

    /// Get reference to `Submit` instance by id.
    ///
    /// # Panic
    ///
    /// This function will panic if requested submit isn't part of this family.
    ///
    pub fn get_submit(&self, sid: SubmitId) -> Option<&Submit> {
        assert_eq!(self.id, sid.family());
        self.get_queue(sid.queue())
            .and_then(|queue| queue.get_submit(sid))
    }

    /// Get mutable reference to `Submit` instance by id.
    ///
    /// # Panic
    ///
    /// This function will panic if requested submit isn't part of this family.
    ///
    pub fn get_submit_mut(&mut self, sid: SubmitId) -> Option<&mut Submit> {
        assert_eq!(self.id, sid.family());
        self.get_queue_mut(sid.queue())
            .and_then(|queue| queue.get_submit_mut(sid))
    }
}

impl Index<QueueId> for Family {
    type Output = Queue;

    fn index(&self, qid: QueueId) -> &Queue {
        self.get_queue(qid).unwrap()
    }
}

impl IndexMut<QueueId> for Family {
    fn index_mut(&mut self, qid: QueueId) -> &mut Queue {
        self.get_queue_mut(qid).unwrap()
    }
}

impl Index<SubmitId> for Family {
    type Output = Submit;

    fn index(&self, sid: SubmitId) -> &Submit {
        self.get_submit(sid).unwrap()
    }
}

impl IndexMut<SubmitId> for Family {
    fn index_mut(&mut self, sid: SubmitId) -> &mut Submit {
        self.get_submit_mut(sid).unwrap()
    }
}

/// All families on which passes were scheduled.
pub struct Families {
    map: HashMap<QueueFamilyId, Family>,
}

impl Families {
    /// Iterate over all families.
    pub fn iter(&self) -> HashMapIter<QueueFamilyId, Family> {
        self.map.iter()
    }

    /// Get reference to `Family` instance by the id.
    pub fn get_family(&self, fid: QueueFamilyId) -> Option<&Family> {
        self.map.get(&fid)
    }

    /// Get mutable reference to `Family` instance by the id.
    pub fn get_family_mut(&mut self, fid: QueueFamilyId) -> Option<&mut Family> {
        self.map.get_mut(&fid)
    }

    /// Get mutable reference to `Family` instance by the id.
    /// This function will add empty `Family` if id is not present.
    pub fn ensure_family(&mut self, fid: QueueFamilyId) -> &mut Family {
        self.map.entry(fid).or_insert_with(|| Family::new(fid))
    }

    /// Get reference to `Queue` instance by the id.
    pub fn get_queue(&self, qid: QueueId) -> Option<&Queue> {
        self.get_family(qid.family())
            .and_then(|family| family.get_queue(qid))
    }

    /// Get mutable reference to `Queue` instance by the id.
    pub fn get_queue_mut(&mut self, qid: QueueId) -> Option<&mut Queue> {
        self.get_family_mut(qid.family())
            .and_then(|family| family.get_queue_mut(qid))
    }

    /// Get mutable reference to `Queue` instance by the id.
    /// This function will grow queues array if index is out of bounds.
    pub fn ensure_queue(&mut self, qid: QueueId) -> &mut Queue {
        self.ensure_family(qid.family()).ensure_queue(qid)
    }

    /// Get reference to `Submit` instance by id.
    pub fn get_submit(&self, sid: SubmitId) -> Option<&Submit> {
        self.get_queue(sid.queue())
            .and_then(|queue| queue.get_submit(sid))
    }

    /// Get reference to `Submit` instance by id.
    pub fn get_submit_mut(&mut self, sid: SubmitId) -> Option<&mut Submit> {
        self.get_queue_mut(sid.queue())
            .and_then(|queue| queue.get_submit_mut(sid))
    }
}

impl Default for Families {
    fn default() -> Self {
        Families {
            map: HashMap::default(),
        }
    }
}

impl Index<QueueFamilyId> for Families {
    type Output = Family;

    fn index(&self, fid: QueueFamilyId) -> &Family {
        self.get_family(fid).unwrap()
    }
}

impl IndexMut<QueueFamilyId> for Families {
    fn index_mut(&mut self, fid: QueueFamilyId) -> &mut Family {
        self.get_family_mut(fid).unwrap()
    }
}

impl Index<QueueId> for Families {
    type Output = Queue;

    fn index(&self, qid: QueueId) -> &Queue {
        self.get_queue(qid).unwrap()
    }
}

impl IndexMut<QueueId> for Families {
    fn index_mut(&mut self, qid: QueueId) -> &mut Queue {
        self.get_queue_mut(qid).unwrap()
    }
}

impl Index<SubmitId> for Families {
    type Output = Submit;

    fn index(&self, sid: SubmitId) -> &Submit {
        self.get_submit(sid).unwrap()
    }
}

impl IndexMut<SubmitId> for Families {
    fn index_mut(&mut self, sid: SubmitId) -> &mut Submit {
        self.get_submit_mut(sid).unwrap()
    }
}

/// Allows to insert links to submit generically.
pub(crate) trait SubmitInsertLink<R> {
    /// Insert new link into submit.
    fn insert_link(&mut self, id: Id<R>, index: usize);
}

impl SubmitInsertLink<Buffer> for Submit {
    fn insert_link(&mut self, id: Id<Buffer>, index: usize) {
        self.buffers.insert(id, index);
    }
}

impl SubmitInsertLink<Image> for Submit {
    fn insert_link(&mut self, id: Id<Image>, index: usize) {
        self.images.insert(id, index);
    }
}
