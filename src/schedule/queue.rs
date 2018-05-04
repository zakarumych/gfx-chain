use std::iter::{DoubleEndedIterator, Enumerate, ExactSizeIterator};
use std::ops::{Index, IndexMut};
use std::slice::{Iter as SliceIter, IterMut as SliceIterMut};
use std::vec::{IntoIter as VecIntoIter};

use hal::queue::QueueFamilyId;

use super::submission::{Submission, SubmissionId};

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

/// Iterator over references to submissions in queue.
#[derive(Debug, Clone)]
pub struct QueueIter<'a, S: 'a> {
    qid: QueueId,
    iter: Enumerate<SliceIter<'a, Submission<S>>>,
}

impl<'a, S> Iterator for QueueIter<'a, S> {
    type Item = (SubmissionId, &'a Submission<S>);

    fn next(&mut self) -> Option<(SubmissionId, &'a Submission<S>)> {
        self.iter
            .next()
            .map(|(index, submission)| (SubmissionId::new(self.qid, index), submission))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.iter.size_hint()
    }
}

impl<'a, S> DoubleEndedIterator for QueueIter<'a, S> {
    fn next_back(&mut self) -> Option<(SubmissionId, &'a Submission<S>)> {
        self.iter
            .next_back()
            .map(|(index, submission)| (SubmissionId::new(self.qid, index), submission))
    }
}

impl<'a, S> ExactSizeIterator for QueueIter<'a, S> {}

/// Iterator over mutable references to submissions in queue.
#[derive(Debug)]
pub struct QueueIterMut<'a, S: 'a> {
    qid: QueueId,
    iter: Enumerate<SliceIterMut<'a, Submission<S>>>,
}

impl<'a, S> Iterator for QueueIterMut<'a, S> {
    type Item = (SubmissionId, &'a mut Submission<S>);

    fn next(&mut self) -> Option<(SubmissionId, &'a mut Submission<S>)> {
        self.iter
            .next()
            .map(|(index, submission)| (SubmissionId::new(self.qid, index), submission))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.iter.size_hint()
    }
}

impl<'a, S> DoubleEndedIterator for QueueIterMut<'a, S> {
    fn next_back(&mut self) -> Option<(SubmissionId, &'a mut Submission<S>)> {
        self.iter
            .next_back()
            .map(|(index, submission)| (SubmissionId::new(self.qid, index), submission))
    }
}

impl<'a, S> ExactSizeIterator for QueueIterMut<'a, S> {}

/// Iterator over owned references to submissions in queue.
#[derive(Debug)]
pub struct QueueIntoIter<S> {
    qid: QueueId,
    iter: Enumerate<VecIntoIter<Submission<S>>>,
}

impl<S> Iterator for QueueIntoIter<S> {
    type Item = (SubmissionId, Submission<S>);

    fn next(&mut self) -> Option<(SubmissionId, Submission<S>)> {
        self.iter
            .next()
            .map(|(index, submission)| (SubmissionId::new(self.qid, index), submission))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.iter.size_hint()
    }
}

impl<S> DoubleEndedIterator for QueueIntoIter<S> {
    fn next_back(&mut self) -> Option<(SubmissionId, Submission<S>)> {
        self.iter
            .next_back()
            .map(|(index, submission)| (SubmissionId::new(self.qid, index), submission))
    }
}

impl<S> ExactSizeIterator for QueueIntoIter<S> {}

/// Instances of this type contains array of `Submission`s.
/// Those submissions are expected to be submissionted in order.
#[derive(Clone, Debug)]
pub struct Queue<S> {
    id: QueueId,
    submissions: Vec<Submission<S>>,
}

impl<S> Queue<S> {
    /// Create new queue with specified id.
    pub fn new(id: QueueId) -> Self {
        Queue {
            id,
            submissions: Vec::default(),
        }
    }

    /// Get id of the queue.
    pub fn id(&self) -> QueueId {
        self.id
    }

    /// Iterate over immutable references to each submission in this queue
    pub fn iter(&self) -> QueueIter<S> {
        QueueIter {
            qid: self.id,
            iter: self.submissions.iter().enumerate(),
        }
    }

    /// Iterate over mutable references to each submission in this queue
    pub fn iter_mut(&mut self) -> QueueIterMut<S> {
        QueueIterMut {
            qid: self.id,
            iter: self.submissions.iter_mut().enumerate(),
        }
    }

    /// Iterate over mutable references to each submission in this queue
    pub fn into_iter(self) -> QueueIntoIter<S> {
        QueueIntoIter {
            qid: self.id,
            iter: self.submissions.into_iter().enumerate(),
        }
    }

    /// Get the number of submissions in queue.
    pub fn len(&self) -> usize {
        self.submissions.len()
    }

    /// Get reference to `Submission` instance by id.
    ///
    /// # Panic
    ///
    /// This function will panic if requested submission isn't part of this queue.
    ///
    pub fn submission(&self, sid: SubmissionId) -> Option<&Submission<S>> {
        assert_eq!(self.id, sid.queue());
        self.submissions.get(sid.index())
    }

    /// Get mutable reference to `Submission` instance by id.
    ///
    /// # Panic
    ///
    /// This function will panic if requested submission isn't part of this queue.
    ///
    pub fn submission_mut(&mut self, sid: SubmissionId) -> Option<&mut Submission<S>> {
        assert_eq!(self.id, sid.queue());
        self.submissions.get_mut(sid.index())
    }

    /// Get reference to last `Submission` instance.
    pub fn last_submission(&self) -> Option<&Submission<S>> {
        self.submissions.last()
    }

    /// Get mutable reference to last `Submission` instance.
    pub fn last_submission_mut(&mut self) -> Option<&mut Submission<S>> {
        self.submissions.last_mut()
    }

    /// Add `Submission` instance to the end of queue.
    /// Returns id of the added submission.
    pub fn add_submission(&mut self, submission: Submission<S>) -> SubmissionId {
        self.submissions.push(submission);
        SubmissionId::new(self.id, self.submissions.len() - 1)
    }
}

impl<S> IntoIterator for Queue<S> {
    type Item = (SubmissionId, Submission<S>);
    type IntoIter = QueueIntoIter<S>;

    fn into_iter(self) -> QueueIntoIter<S> {
        self.into_iter()
    }
}

impl<'a, S> IntoIterator for &'a Queue<S> {
    type Item = (SubmissionId, &'a Submission<S>);
    type IntoIter = QueueIter<'a, S>;

    fn into_iter(self) -> QueueIter<'a, S> {
        self.iter()
    }
}

impl<'a, S> IntoIterator for &'a mut Queue<S> {
    type Item = (SubmissionId, &'a mut Submission<S>);
    type IntoIter = QueueIterMut<'a, S>;

    fn into_iter(self) -> QueueIterMut<'a, S> {
        self.iter_mut()
    }
}

impl<S> Index<SubmissionId> for Queue<S> {
    type Output = Submission<S>;

    fn index(&self, sid: SubmissionId) -> &Submission<S> {
        self.submission(sid).unwrap()
    }
}

impl<S> IndexMut<SubmissionId> for Queue<S> {
    fn index_mut(&mut self, sid: SubmissionId) -> &mut Submission<S> {
        self.submission_mut(sid).unwrap()
    }
}
