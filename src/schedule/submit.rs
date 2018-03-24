use std::collections::HashMap;

use hal::queue::QueueFamilyId;

use Pick;
use resource::{Buffer, Id, Image};
use pass::PassId;

use super::QueueId;

/// Submit id.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SubmitId {
    queue: QueueId,
    index: usize,
}

impl SubmitId {
    /// Create new id from queue id and index.
    pub fn new(queue: QueueId, index: usize) -> Self {
        SubmitId { queue, index }
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
pub struct Submit<S> {
    pub(crate) buffers: HashMap<Id<Buffer>, usize>,
    pub(crate) images: HashMap<Id<Image>, usize>,
    pub(crate) pass: PassId,
    pub(crate) wait_factor: usize,
    pub(crate) sync: S,
}

impl<S> Submit<S> {
    /// Create new submit with specified pass.
    pub fn new(wait_factor: usize, pass: PassId, sync: S) -> Self {
        Submit {
            buffers: HashMap::new(),
            images: HashMap::new(),
            pass,
            wait_factor,
            sync,
        }
    }

    /// Set new synchronization to the `Submit`.
    pub fn set_sync<T>(&self, sync: T) -> Submit<T> {
        Submit {
            buffers: self.buffers.clone(),
            images: self.images.clone(),
            pass: self.pass,
            wait_factor: self.wait_factor,
            sync,
        }
    }
}

impl<S> Pick<Buffer> for Submit<S> {
    type Target = HashMap<Id<Buffer>, usize>;

    fn pick(&self) -> &HashMap<Id<Buffer>, usize> {
        &self.buffers
    }
    fn pick_mut(&mut self) -> &mut HashMap<Id<Buffer>, usize> {
        &mut self.buffers
    }
}

impl<S> Pick<Image> for Submit<S> {
    type Target = HashMap<Id<Image>, usize>;

    fn pick(&self) -> &HashMap<Id<Image>, usize> {
        &self.images
    }
    fn pick_mut(&mut self) -> &mut HashMap<Id<Image>, usize> {
        &mut self.images
    }
}
