use std::ops::{Index, IndexMut};
use std::slice::{Iter as SliceIter, IterMut as SliceIterMut};

use hal::queue::QueueFamilyId;

use super::submission::{Submission, SubmissionId};
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
    pub fn queue(&self, qid: QueueId) -> Option<&Queue<S>> {
        assert_eq!(self.id, qid.family());
        self.queues.get(qid.index())
    }

    /// Get mutable reference to `Queue` instance by the id.
    ///
    /// # Panic
    ///
    /// This function will panic if requested queue isn't part of this family.
    ///
    pub fn queue_mut(&mut self, qid: QueueId) -> Option<&mut Queue<S>> {
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
            .extend((len..qid.index() + 1).map(|i| Queue::new(QueueId::new(qid.family(), i))));
        &mut self.queues[qid.index()]
    }

    /// Get reference to `Submission<S>` instance by id.
    ///
    /// # Panic
    ///
    /// This function will panic if requested submission isn't part of this family.
    ///
    pub fn submission(&self, sid: SubmissionId) -> Option<&Submission<S>> {
        assert_eq!(self.id, sid.family());
        self.queue(sid.queue())
            .and_then(|queue| queue.submission(sid))
    }

    /// Get mutable reference to `Submission<S>` instance by id.
    ///
    /// # Panic
    ///
    /// This function will panic if requested submission isn't part of this family.
    ///
    pub fn submission_mut(&mut self, sid: SubmissionId) -> Option<&mut Submission<S>> {
        assert_eq!(self.id, sid.family());
        self.queue_mut(sid.queue())
            .and_then(|queue| queue.submission_mut(sid))
    }
}

impl<S> Index<QueueId> for Family<S> {
    type Output = Queue<S>;

    fn index(&self, qid: QueueId) -> &Queue<S> {
        self.queue(qid).unwrap()
    }
}

impl<S> IndexMut<QueueId> for Family<S> {
    fn index_mut(&mut self, qid: QueueId) -> &mut Queue<S> {
        self.queue_mut(qid).unwrap()
    }
}

impl<S> Index<SubmissionId> for Family<S> {
    type Output = Submission<S>;

    fn index(&self, sid: SubmissionId) -> &Submission<S> {
        self.submission(sid).unwrap()
    }
}

impl<S> IndexMut<SubmissionId> for Family<S> {
    fn index_mut(&mut self, sid: SubmissionId) -> &mut Submission<S> {
        self.submission_mut(sid).unwrap()
    }
}
