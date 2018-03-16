use std::collections::hash_map::{HashMap, Iter as HashMapIter};
use hal::queue::QueueFamilyId;
use resource::{Buffer, Id, Image, State};

#[derive(Copy, Clone, Debug, PartialEq, PartialOrd, Eq, Ord)]
pub struct PassId(pub usize);

#[derive(Clone, Debug)]
pub struct Pass {
    family: QueueFamilyId,
    queue: Option<usize>,
    dependencies: Vec<usize>,
    buffers: HashMap<Id<Buffer>, State<Buffer>>,
    images: HashMap<Id<Image>, State<Image>>,
}

impl Pass {
    pub fn family(&self) -> QueueFamilyId {
        self.family
    }

    pub fn queue(&self) -> Option<usize> {
        self.queue
    }

    pub fn dependencies(&self) -> &[usize] {
        &self.dependencies
    }

    pub fn buffers(&self) -> HashMapIter<Id<Buffer>, State<Buffer>> {
        self.buffers.iter()
    }

    pub fn images(&self) -> HashMapIter<Id<Image>, State<Image>> {
        self.images.iter()
    }
}
