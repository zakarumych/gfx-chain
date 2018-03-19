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
    pub(crate) pass: PassId,
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
            pass,
            wait_factor,
        }
    }
}
