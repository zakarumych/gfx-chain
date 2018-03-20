mod submit;

use std::iter::Enumerate;
use std::ops::{Index, IndexMut};
use std::slice::{Iter as SliceIter, IterMut as SliceIterMut};

use hal::queue::QueueFamilyId;

pub use self::submit::{Submit, SubmitId};

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

pub struct Submits<'a> {
    qid: QueueId,
    iter: Enumerate<SliceIter<'a, Submit>>,
}

impl<'a> Iterator for Submits<'a> {
    type Item = (SubmitId, &'a Submit);

    fn next(&mut self) -> Option<(SubmitId, &'a Submit)> {
        self.iter
            .next()
            .map(|(index, submit)| (SubmitId::new(self.qid, index), submit))
    }
}

pub struct SubmitsMut<'a> {
    qid: QueueId,
    iter: Enumerate<SliceIterMut<'a, Submit>>,
}

impl<'a> Iterator for SubmitsMut<'a> {
    type Item = (SubmitId, &'a mut Submit);

    fn next(&mut self) -> Option<(SubmitId, &'a mut Submit)> {
        self.iter
            .next()
            .map(|(index, submit)| (SubmitId::new(self.qid, index), submit))
    }
}

/// Instances of this type contains array of `Submit`s.
/// Those submits are expected to be submitted in order.
#[derive(Clone, Debug)]
pub struct Queue {
    id: QueueId,
    submits: Vec<Submit>,
}

impl Queue {
    /// Create new queue with specified id.
    pub fn new(id: QueueId) -> Self {
        Queue {
            id,
            submits: Vec::default(),
        }
    }

    /// Iterate over references to all submits.
    pub fn iter(&self) -> Submits {
        Submits {
            qid: self.id,
            iter: self.submits.iter().enumerate(),
        }
    }

    /// Iterate over mutable references to all submits.
    pub fn iter_mut(&mut self) -> SubmitsMut {
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
    pub fn get_submit(&self, sid: SubmitId) -> Option<&Submit> {
        assert_eq!(self.id, sid.queue());
        self.submits.get(sid.index())
    }

    /// Get mutable reference to `Submit` instance by id.
    ///
    /// # Panic
    ///
    /// This function will panic if requested submit isn't part of this queue.
    ///
    pub fn get_submit_mut(&mut self, sid: SubmitId) -> Option<&mut Submit> {
        assert_eq!(self.id, sid.queue());
        self.submits.get_mut(sid.index())
    }

    /// Get reference to last `Submit` instance.
    pub fn last_submit(&self) -> Option<&Submit> {
        self.submits.last()
    }

    /// Get mutable reference to last `Submit` instance.
    pub fn last_submit_mut(&mut self) -> Option<&mut Submit> {
        self.submits.last_mut()
    }

    /// Add `Submit` instance to the end of queue.
    /// Returns id of the added submit.
    pub fn add_submit(&mut self, submit: Submit) -> SubmitId {
        self.submits.push(submit);
        SubmitId::new(self.id, self.submits.len() - 1)
    }
}

impl Index<SubmitId> for Queue {
    type Output = Submit;

    fn index(&self, sid: SubmitId) -> &Submit {
        self.get_submit(sid).unwrap()
    }
}

impl IndexMut<SubmitId> for Queue {
    fn index_mut(&mut self, sid: SubmitId) -> &mut Submit {
        self.get_submit_mut(sid).unwrap()
    }
}
