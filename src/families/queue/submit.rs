use std::collections::HashMap;

use hal::pso::PipelineStage;
use hal::queue::QueueFamilyId;

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
pub struct Submit {
    pub(crate) wait: Vec<(SubmitId, PipelineStage)>,
    pub(crate) signal: Vec<(SubmitId, PipelineStage)>,
    pub(crate) buffers: HashMap<Id<Buffer>, usize>,
    pub(crate) images: HashMap<Id<Image>, usize>,
    pub(crate) pass: Option<PassId>,
    pub(crate) wait_factor: usize,
}

impl Submit {
    /// Create new submit with specified pass.
    pub fn new(wait_factor: usize, pass: PassId) -> Self {
        Submit {
            wait: Vec::new(),
            signal: Vec::new(),
            buffers: HashMap::new(),
            images: HashMap::new(),
            pass: Some(pass),
            wait_factor,
        }
    }

    /// Create new submit with no pass.
    /// This submit will acquire or release resources.
    pub fn new_transfer(wait_factor: usize) -> Self {
        Submit {
            wait: Vec::new(),
            signal: Vec::new(),
            buffers: HashMap::new(),
            images: HashMap::new(),
            pass: None,
            wait_factor,
        }
    }

    /// Check if this submit is of transfer type
    pub fn is_transfer(&self) -> bool {
        self.pass.is_none()
    }
}

pub trait SubmitInsertLink<R> {
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
