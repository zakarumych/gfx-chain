//!
//! This module defines `Pass` type that contains information required to
//! synchronize execution of passes.
//!

use std::collections::hash_map::{HashMap, Iter as HashMapIter};
use hal::queue::QueueFamilyId;
use resource::{Buffer, Id, Image, State, Resource};

/// Id of the pass.
#[derive(Copy, Clone, Debug, PartialEq, PartialOrd, Eq, Ord)]
pub struct PassId(pub usize);

/// State in which pass uses resource and usage flags.
#[derive(Clone, Copy, Debug)]
pub struct StateUsage<R: Resource> {
    /// State in which pass uses resource.
    pub state: State<R>,

    /// Usage flags.
    pub usage: R::Usage,
}

/// Description of pass.
#[derive(Clone, Debug)]
pub struct Pass {
    /// Id of the pass.
    pub id: PassId,

    /// Family required to execute the pass.
    pub family: QueueFamilyId,

    /// Specific queue for the pass. Or `None` if any will do.
    pub queue: Option<usize>,

    /// Dependencies of the pass.
    /// Those are indices of other passes in array.
    pub dependencies: Vec<PassId>,

    /// Buffer category ids and required state.
    pub buffers: HashMap<Id<Buffer>, StateUsage<Buffer>>,

    /// Image category ids and required state.
    pub images: HashMap<Id<Image>, StateUsage<Image>>,
}

impl Pass {
    /// Get family on which this pass will be executed.
    pub fn family(&self) -> QueueFamilyId {
        self.family
    }

    /// Get queue to which this pass assigned. Or `None`.
    pub fn queue(&self) -> Option<usize> {
        self.queue
    }

    /// Get indices of passes this pass depends on.
    pub fn dependencies(&self) -> &[PassId] {
        &self.dependencies
    }

    /// Get iterator to buffer states this pass accesses.
    pub fn buffers(&self) -> HashMapIter<Id<Buffer>, StateUsage<Buffer>> {
        self.buffers.iter()
    }

    /// Get iterator to image states this pass accesses.
    pub fn images(&self) -> HashMapIter<Id<Image>, StateUsage<Image>> {
        self.images.iter()
    }
}
