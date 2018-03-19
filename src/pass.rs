use std::collections::hash_map::{HashMap, Iter as HashMapIter};
use hal::queue::QueueFamilyId;
use resource::{Buffer, Id, Image, State};

/// Id of the pass.
#[derive(Copy, Clone, Debug, PartialEq, PartialOrd, Eq, Ord)]
pub struct PassId(pub usize);

/// Description of pass.
#[derive(Clone, Debug)]
pub struct Pass {
    /// Family required to execute the pass.
    pub family: QueueFamilyId,

    /// Specific queue for the pass. Or `None` if any will do.
    pub queue: Option<usize>,

    /// Dependencies of the pass.
    /// Those are indices of other passes in array.
    pub dependencies: Vec<usize>,

    /// Buffer category ids and required state.
    pub buffers: HashMap<Id<Buffer>, State<Buffer>>,

    /// Image category ids and required state.
    pub images: HashMap<Id<Image>, State<Image>>,
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
