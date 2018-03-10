use hal::queue::QueueFamilyId;

/// Unique identifier of the queue.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct QueueId {
    index: usize,
    family: QueueFamilyId,
}

impl QueueId {
    /// Create queue identifier from index in queue group and family id.
    pub fn new(index: usize, family: QueueFamilyId) -> Self {
        QueueId { index, family }
    }

    /// Get index in queue group.
    pub fn index(&self) -> usize {
        self.index
    }

    /// Get family id.
    pub fn family(&self) -> QueueFamilyId {
        self.family
    }
}
