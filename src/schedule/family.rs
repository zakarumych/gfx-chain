use std::ops::{Index, IndexMut};
use std::slice::{Iter as SliceIter, IterMut as SliceIterMut};

use hal::queue::QueueFamilyId;

use super::submit::{Submit, SubmitId};
use super::queue::{Queue, QueueId};

/// Instances of this type contains array of `Queue`s.
/// All contained queues has identical capabilities.
#[derive(Clone, Debug)]
pub struct Family<S> {
    id: QueueFamilyId,
    queues: Vec<Queue<S>>,
}

impl<S> Family<S> {
    /// Create new empty `Family`
    pub fn new(id: QueueFamilyId) -> Self {
        Family {
            id,
            queues: Vec::default(),
        }
    }

    /// Get iterator over references to queues
    pub fn iter(&self) -> SliceIter<Queue<S>> {
        self.queues.iter()
    }

    /// Get iterator over mutable references to queues
    pub fn iter_mut(&mut self) -> SliceIterMut<Queue<S>> {
        self.queues.iter_mut()
    }

    /// Get reference to `Queue` instance by the id.
    ///
    /// # Panic
    ///
    /// This function will panic if requested queue isn't part of this family.
    ///
    pub fn get_queue(&self, qid: QueueId) -> Option<&Queue<S>> {
        assert_eq!(self.id, qid.family());
        self.queues.get(qid.index())
    }

    /// Get mutable reference to `Queue` instance by the id.
    ///
    /// # Panic
    ///
    /// This function will panic if requested queue isn't part of this family.
    ///
    pub fn get_queue_mut(&mut self, qid: QueueId) -> Option<&mut Queue<S>> {
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
    pub fn ensure_queue(&mut self, qid: QueueId) -> &mut Queue<S> {
        assert_eq!(self.id, qid.family());
        let len = self.queues.len();
        self.queues
            .extend((len..qid.index() + 1).map(|_| Queue::new(qid)));
        &mut self.queues[qid.index()]
    }

    /// Get reference to `Submit<S>` instance by id.
    ///
    /// # Panic
    ///
    /// This function will panic if requested submit isn't part of this family.
    ///
    pub fn get_submit(&self, sid: SubmitId) -> Option<&Submit<S>> {
        assert_eq!(self.id, sid.family());
        self.get_queue(sid.queue())
            .and_then(|queue| queue.get_submit(sid))
    }

    /// Get mutable reference to `Submit<S>` instance by id.
    ///
    /// # Panic
    ///
    /// This function will panic if requested submit isn't part of this family.
    ///
    pub fn get_submit_mut(&mut self, sid: SubmitId) -> Option<&mut Submit<S>> {
        assert_eq!(self.id, sid.family());
        self.get_queue_mut(sid.queue())
            .and_then(|queue| queue.get_submit_mut(sid))
    }
}

impl<S> Index<QueueId> for Family<S> {
    type Output = Queue<S>;

    fn index(&self, qid: QueueId) -> &Queue<S> {
        self.get_queue(qid).unwrap()
    }
}

impl<S> IndexMut<QueueId> for Family<S> {
    fn index_mut(&mut self, qid: QueueId) -> &mut Queue<S> {
        self.get_queue_mut(qid).unwrap()
    }
}

impl<S> Index<SubmitId> for Family<S> {
    type Output = Submit<S>;

    fn index(&self, sid: SubmitId) -> &Submit<S> {
        self.get_submit(sid).unwrap()
    }
}

impl<S> IndexMut<SubmitId> for Family<S> {
    fn index_mut(&mut self, sid: SubmitId) -> &mut Submit<S> {
        self.get_submit_mut(sid).unwrap()
    }
}
