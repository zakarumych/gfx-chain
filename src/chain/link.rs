use std::collections::hash_map::{Entry, HashMap, Iter as HashMapIter};

use hal::pso::PipelineStage;
use hal::queue::QueueFamilyId;

use resource::{Resource, State};
use schedule::{QueueId, SubmissionId};

/// State of the link associated with queue.
/// Contains submissions range, combined access and stages bits by submissions from the range.
#[derive(Clone, Debug)]
pub(crate) struct LinkQueueState<R: Resource> {
    first: usize,
    last: usize,
    access: R::Access,
    stages: PipelineStage,
}

impl<R> LinkQueueState<R>
where
    R: Resource,
{
    fn new(sid: SubmissionId, state: State<R>) -> Self {
        LinkQueueState {
            first: sid.index(),
            last: sid.index(),
            access: state.access,
            stages: state.stages,
        }
    }

    fn push(&mut self, sid: SubmissionId, state: State<R>) {
        self.access |= state.access;
        self.stages |= state.stages;
        self.last = sid.index();
    }

    /// First submission index.
    pub fn first(&self) -> usize {
        self.first
    }

    /// Last submission index.
    pub fn last(&self) -> usize {
        self.last
    }
}

/// This type defines what states resource are at some point in time when commands recorded into
/// corresponding submissions are executed.
/// Those commands doesn't required to perform actions with all access types declared by the link.
/// But performing actions with access types not declared by the link is prohibited.
#[derive(Clone, Debug)]
pub struct Link<R: Resource> {
    usage: R::Usage,
    state: State<R>,
    queues: HashMap<usize, LinkQueueState<R>>,
    family: QueueFamilyId,
}

impl<R> Link<R>
where
    R: Resource,
{
    /// Create new link with first attached submission.
    ///
    /// # Parameters
    ///
    /// `state`     - state of the first submission.
    /// `sid`       - id of the first submission.
    ///
    pub fn new(sid: SubmissionId, state: State<R>, usage: R::Usage) -> Self {
        use std::iter::once;
        Link {
            state,
            queues: once((sid.queue().index(), LinkQueueState::new(sid, state))).collect(),
            family: sid.family(),
            usage,
        }
    }

    /// Get queue family that owns the resource at the link.
    /// All associated submissions must be from the same queue family.
    pub fn family(&self) -> QueueFamilyId {
        self.family
    }

    /// Get usage.
    pub fn usage(&self) -> R::Usage {
        self.usage
    }

    /// Get state.
    pub fn state(&self) -> State<R> {
        self.state
    }

    /// Check if the link is associated with only one submission.
    pub fn exclusive(&self) -> bool {
        let mut values = self.queues.values();
        let queue = values.next().unwrap();
        values.next().is_none() && queue.first() == queue.last()
    }

    /// Check if the link is associated with only one queue.
    pub fn single_queue(&self) -> bool {
        self.queues().len() == 1
    }

    /// Check if the given state and submission are compatible with link.
    /// If compatible then the submission can be associated with the link.
    pub fn compatible(&self, sid: SubmissionId, state: State<R>) -> bool {
        // If queue the same and states are compatible.
        self.family == sid.family() && self.state.compatible(state)
    }

    /// Insert submission with specified state to the link.
    /// It must be compatible.
    /// Associating submission with the link will allow the submission
    /// to be executed in parallel with other submissions associated with this link.
    /// Unless other chains disallow.
    ///
    /// # Panics
    ///
    /// This function will panic if `state` and `sid` are not compatible.
    /// E.g. `Link::compatible` didn't returned `true` for the arguments.
    ///
    pub fn insert_submission(&mut self, sid: SubmissionId, state: State<R>, usage: R::Usage) {
        assert_eq!(self.family, sid.family());
        let state = self.state.merge(state);
        match self.queues.entry(sid.queue().index()) {
            Entry::Occupied(mut occupied) => {
                occupied.get_mut().push(sid, state);
            }
            Entry::Vacant(vacant) => {
                vacant.insert(LinkQueueState::new(sid, state));
            }
        };
        self.state = state;
        self.usage |= usage;
    }

    /// Collect first submissions from all queues.
    pub fn heads(&self) -> Vec<SubmissionId> {
        self.queues
            .iter()
            .map(|(&index, queue)| SubmissionId::new(QueueId::new(self.family, index), queue.first()))
            .collect()
    }

    /// Collect last submissions from all queues.
    pub fn tails(&self) -> Vec<SubmissionId> {
        self.queues
            .iter()
            .map(|(&index, queue)| SubmissionId::new(QueueId::new(self.family, index), queue.last()))
            .collect()
    }

    /// Check if ownership transfer is required between those links.
    pub fn transfer(&self, next: &Self) -> bool {
        self.family != next.family
    }

    /// Get iterator over queue
    pub(crate) fn queues(&self) -> QueuesIter<R> {
        QueuesIter {
            family: self.family,
            iter: self.queues.iter(),
        }
    }

    ///
    pub(crate) fn queue_state(&self, qid: QueueId) -> State<R> {
        let queue = self.queue(qid).unwrap();
        State {
            access: queue.access,
            stages: queue.stages,
            .. self.state
        }
    }

    pub(crate) fn queue(&self, qid: QueueId) -> Option<&LinkQueueState<R>> {
        assert_eq!(self.family, qid.family());
        self.queues.get(&qid.index())
    }
}

pub(crate) struct QueuesIter<'a, R: Resource + 'a> {
    family: QueueFamilyId,
    iter: HashMapIter<'a, usize, LinkQueueState<R>>,
}

impl<'a, R> Iterator for QueuesIter<'a, R>
where
    R: Resource + 'a,
{
    type Item = (QueueId, &'a LinkQueueState<R>);

    fn next(&mut self) -> Option<(QueueId, &'a LinkQueueState<R>)> {
        self.iter
            .next()
            .map(|(&index, queue)| (QueueId::new(self.family, index), queue))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.iter.size_hint()
    }
}

impl<'a, R> ExactSizeIterator for QueuesIter<'a, R>
where
    R: Resource + 'a,
{
}
