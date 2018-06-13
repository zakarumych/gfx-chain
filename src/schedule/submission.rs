use fnv::FnvHashMap;
use std::collections::hash_map::{Iter as HashMapIter};

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
    buffers: FnvHashMap<Id<Buffer>, usize>,
    images: FnvHashMap<Id<Image>, usize>,
    pass: PassId,
    wait_factor: usize,
    submit_order: usize,
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

    /// Get wait factor for `Submission`
    pub fn wait_factor(&self) -> usize {
        self.wait_factor
    }

    /// Get submit order for `Submission`
    pub fn submit_order(&self) -> usize {
        self.submit_order
    }

    /// Iterator over buffers
    pub fn buffers(&self) -> HashMapIter<Id<Buffer>, usize> {
        self.buffers.iter()
    }

    /// Iterator over images
    pub fn images(&self) -> HashMapIter<Id<Image>, usize> {
        self.images.iter()
    }

    /// Get link index for buffer by id.
    pub fn buffer(&self, id: Id<Buffer>) -> usize {
        self.buffers[&id]
    }

    /// Get link index for image by id.
    pub fn image(&self, id: Id<Image>) -> usize {
        self.images[&id]
    }

    /// Create new submission with specified pass.
    pub(crate) fn new(wait_factor: usize, submit_order: usize, pass: PassId, sync: S) -> Self {
        Submission {
            buffers: FnvHashMap::default(),
            images: FnvHashMap::default(),
            pass,
            wait_factor,
            submit_order,
            sync,
        }
    }

    /// Set new synchronization to the `Submission`.
    pub(crate) fn set_sync<T>(&self, sync: T) -> Submission<T> {
        Submission {
            buffers: self.buffers.clone(),
            images: self.images.clone(),
            pass: self.pass,
            wait_factor: self.wait_factor,
            submit_order: self.submit_order,
            sync,
        }
    }
}

impl<S> Pick<Buffer> for Submission<S> {
    type Target = FnvHashMap<Id<Buffer>, usize>;

    fn pick(&self) -> &FnvHashMap<Id<Buffer>, usize> {
        &self.buffers
    }
    fn pick_mut(&mut self) -> &mut FnvHashMap<Id<Buffer>, usize> {
        &mut self.buffers
    }
}

impl<S> Pick<Image> for Submission<S> {
    type Target = FnvHashMap<Id<Image>, usize>;

    fn pick(&self) -> &FnvHashMap<Id<Image>, usize> {
        &self.images
    }
    fn pick_mut(&mut self) -> &mut FnvHashMap<Id<Image>, usize> {
        &mut self.images
    }
}
