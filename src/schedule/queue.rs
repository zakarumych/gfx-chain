use std::iter::{DoubleEndedIterator, Enumerate, ExactSizeIterator};
use std::ops::{Index, IndexMut};
use std::slice::{Iter as SliceIter, IterMut as SliceIterMut};

use hal::queue::QueueFamilyId;

use super::submit::{Submit, SubmitId};

/// Queue id.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct QueueId {
    family: usize,
    index: usize,
}

impl QueueId {
    /// Create queue id from family id and index.
    pub fn new(family: QueueFamilyId, index: usize) -> Self {
        QueueId {
            family: family.0,
            index,
        }
    }

    /// Get family id.
    pub fn family(&self) -> QueueFamilyId {
        QueueFamilyId(self.family)
    }

    /// Get index within the family.
    pub fn index(&self) -> usize {
        self.index
    }
}

/// Iterator over references to submits in queue.
#[derive(Debug, Clone)]
pub struct Submits<'a, S: 'a> {
    qid: QueueId,
    iter: Enumerate<SliceIter<'a, Submit<S>>>,
}

impl<'a, S> Iterator for Submits<'a, S> {
    type Item = (SubmitId, &'a Submit<S>);

    fn next(&mut self) -> Option<(SubmitId, &'a Submit<S>)> {
        self.iter
            .next()
            .map(|(index, submit)| (SubmitId::new(self.qid, index), submit))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.iter.size_hint()
    }
}

impl<'a, S> DoubleEndedIterator for Submits<'a, S> {
    fn next_back(&mut self) -> Option<(SubmitId, &'a Submit<S>)> {
        self.iter
            .next_back()
            .map(|(index, submit)| (SubmitId::new(self.qid, index), submit))
    }
}

impl<'a, S> ExactSizeIterator for Submits<'a, S> {}

/// Iterator over mutable references to submits in queue.
#[derive(Debug)]
pub struct SubmitsMut<'a, S: 'a> {
    qid: QueueId,
    iter: Enumerate<SliceIterMut<'a, Submit<S>>>,
}

impl<'a, S> Iterator for SubmitsMut<'a, S> {
    type Item = (SubmitId, &'a mut Submit<S>);

    fn next(&mut self) -> Option<(SubmitId, &'a mut Submit<S>)> {
        self.iter
            .next()
            .map(|(index, submit)| (SubmitId::new(self.qid, index), submit))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.iter.size_hint()
    }
}

impl<'a, S> DoubleEndedIterator for SubmitsMut<'a, S> {
    fn next_back(&mut self) -> Option<(SubmitId, &'a mut Submit<S>)> {
        self.iter
            .next_back()
            .map(|(index, submit)| (SubmitId::new(self.qid, index), submit))
    }
}

impl<'a, S> ExactSizeIterator for SubmitsMut<'a, S> {}

/// Instances of this type contains array of `Submit`s.
/// Those submits are expected to be submitted in order.
#[derive(Clone, Debug)]
pub struct Queue<S> {
    id: QueueId,
    submits: Vec<Submit<S>>,
}

impl<S> Queue<S> {
    /// Create new queue with specified id.
    pub fn new(id: QueueId) -> Self {
        Queue {
            id,
            submits: Vec::default(),
        }
    }

    /// Iterate over references to all submits.
    pub fn iter(&self) -> Submits<S> {
        Submits {
            qid: self.id,
            iter: self.submits.iter().enumerate(),
        }
    }

    /// Iterate over mutable references to all submits.
    pub fn iter_mut(&mut self) -> SubmitsMut<S> {
        SubmitsMut {
            qid: self.id,
            iter: self.submits.iter_mut().enumerate(),
        }
    }

    /// Get the number of submits in queue.
    pub fn len(&self) -> usize {
        self.submits.len()
    }

    /// Get reference to `Submit` instance by id.
    ///
    /// # Panic
    ///
    /// This function will panic if requested submit isn't part of this queue.
    ///
    pub fn get_submit(&self, sid: SubmitId) -> Option<&Submit<S>> {
        assert_eq!(self.id, sid.queue());
        self.submits.get(sid.index())
    }

    /// Get mutable reference to `Submit` instance by id.
    ///
    /// # Panic
    ///
    /// This function will panic if requested submit isn't part of this queue.
    ///
    pub fn get_submit_mut(&mut self, sid: SubmitId) -> Option<&mut Submit<S>> {
        assert_eq!(self.id, sid.queue());
        self.submits.get_mut(sid.index())
    }

    /// Get reference to last `Submit` instance.
    pub fn last_submit(&self) -> Option<&Submit<S>> {
        self.submits.last()
    }

    /// Get mutable reference to last `Submit` instance.
    pub fn last_submit_mut(&mut self) -> Option<&mut Submit<S>> {
        self.submits.last_mut()
    }

    /// Add `Submit` instance to the end of queue.
    /// Returns id of the added submit.
    pub fn add_submit(&mut self, submit: Submit<S>) -> SubmitId {
        self.submits.push(submit);
        SubmitId::new(self.id, self.submits.len() - 1)
    }
}

impl<S> Index<SubmitId> for Queue<S> {
    type Output = Submit<S>;

    fn index(&self, sid: SubmitId) -> &Submit<S> {
        self.get_submit(sid).unwrap()
    }
}

impl<S> IndexMut<SubmitId> for Queue<S> {
    fn index_mut(&mut self, sid: SubmitId) -> &mut Submit<S> {
        self.get_submit_mut(sid).unwrap()
    }
}
