use std::collections::HashMap;

use hal::pso::PipelineStage;
use hal::queue::QueueFamilyId;

use resource::{Buffer, Id, Image};
use pass::PassId;

use super::QueueId;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SubmitId {
    queue: QueueId,
    index: usize,
}

impl SubmitId {
    pub fn new(queue: QueueId, index: usize) -> Self {
        SubmitId { queue, index }
    }

    pub fn family(&self) -> QueueFamilyId {
        self.queue.family()
    }

    pub fn queue(&self) -> QueueId {
        self.queue
    }

    pub fn index(&self) -> usize {
        self.index
    }
}

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

    pub fn empty(wait_factor: usize) -> Self {
        Submit {
            wait: Vec::new(),
            signal: Vec::new(),
            buffers: HashMap::new(),
            images: HashMap::new(),
            pass: None,
            wait_factor,
        }
    }
}

pub trait SubmitInsertLink<R> {
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
