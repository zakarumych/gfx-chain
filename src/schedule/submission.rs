use std::collections::hash_map::{HashMap, Iter as HashMapIter};

use hal::queue::QueueFamilyId;

use Pick;
use resource::{Buffer, Id, Image};
use pass::PassId;

use super::QueueId;

/// Submission id.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SubmissionId {
    queue: QueueId,
    index: usize,
}

impl SubmissionId {
    /// Create new id from queue id and index.
    pub fn new(queue: QueueId, index: usize) -> Self {
        SubmissionId { queue, index }
    }

    /// Get family id.
    pub fn family(&self) -> QueueFamilyId {
        self.queue.family()
    }

    /// Get queue id.
    pub fn queue(&self) -> QueueId {
        self.queue
    }

    /// Get index.
    pub fn index(&self) -> usize {
        self.index
    }
}

/// This type corresponds to commands that should be recorded into single primary command buffer.
#[derive(Clone, Debug)]
pub struct Submission<S> {
    buffers: HashMap<Id<Buffer>, usize>,
    images: HashMap<Id<Image>, usize>,
    pass: PassId,
    wait_factor: usize,
    sync: S,
}

impl<S> Submission<S> {
    /// Get synchronization for `Submission`.
    pub fn pass(&self) -> PassId {
        self.pass
    }

    /// Get synchronization for `Submission`.
    pub fn sync(&self) -> &S {
        &self.sync
    }

    /// Create new submission with specified pass.
    pub(crate) fn new(wait_factor: usize, pass: PassId, sync: S) -> Self {
        Submission {
            buffers: HashMap::new(),
            images: HashMap::new(),
            pass,
            wait_factor,
            sync,
        }
    }

    /// Get wait factor for `Submission`
    pub(crate) fn wait_factor(&self) -> usize {
        self.wait_factor
    }

    /// Iterator over buffers
    pub(crate) fn buffers(&self) -> HashMapIter<Id<Buffer>, usize> {
        self.buffers.iter()
    }

    /// Iterator over images
    pub(crate) fn images(&self) -> HashMapIter<Id<Image>, usize> {
        self.images.iter()
    }

    /// Set new synchronization to the `Submission`.
    pub(crate) fn set_sync<T>(&self, sync: T) -> Submission<T> {
        Submission {
            buffers: self.buffers.clone(),
            images: self.images.clone(),
            pass: self.pass,
            wait_factor: self.wait_factor,
            sync,
        }
    }
}

impl<S> Pick<Buffer> for Submission<S> {
    type Target = HashMap<Id<Buffer>, usize>;

    fn pick(&self) -> &HashMap<Id<Buffer>, usize> {
        &self.buffers
    }
    fn pick_mut(&mut self) -> &mut HashMap<Id<Buffer>, usize> {
        &mut self.buffers
    }
}

impl<S> Pick<Image> for Submission<S> {
    type Target = HashMap<Id<Image>, usize>;

    fn pick(&self) -> &HashMap<Id<Image>, usize> {
        &self.images
    }
    fn pick_mut(&mut self) -> &mut HashMap<Id<Image>, usize> {
        &mut self.images
    }
}
