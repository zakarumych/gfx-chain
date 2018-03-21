use std::collections::hash_map::{Entry, HashMap, Iter as HashMapIter};
use std::ops::Index;

use hal::pso::PipelineStage;
use hal::queue::QueueFamilyId;

use resource::{Resource, State};
use schedule::{QueueId, SubmitId};

#[derive(Clone, Debug)]
pub struct LinkQueueState<R: Resource> {
    first: usize,
    last: usize,
    access: R::Access,
    stages: PipelineStage,
}

impl<R> LinkQueueState<R>
where
    R: Resource,
{
    fn new(sid: SubmitId, state: State<R>) -> Self {
        LinkQueueState {
            first: sid.index(),
            last: sid.index(),
            access: state.access,
            stages: state.stages,
        }
    }

    fn push(&mut self, sid: SubmitId, state: State<R>) {
        assert!(sid.index() == self.last);
        self.access |= state.access;
        self.stages |= state.stages;
        self.last = sid.index() + 1;
    }

    ///
    pub fn first(&self) -> usize {
        self.first
    }

    ///
    pub fn last(&self) -> usize {
        self.last
    }

    ///
    pub fn access(&self) -> R::Access {
        self.access
    }

    ///
    pub fn stages(&self) -> PipelineStage {
        self.stages
    }
}

/// This type defines what states resource are at some point in time when commands recorded into
/// contained submissions are executed.
/// Those commands doesn't required to perform actions with all access types declared by the link.
/// Performing actions with access types not declared by the link is prohibited.
#[derive(Clone, Debug)]
pub struct Link<R: Resource> {
    state: State<R>,
    queues: HashMap<usize, LinkQueueState<R>>,
    family: QueueFamilyId,
}

impl<R> Link<R>
where
    R: Resource,
{
    /// Create new link with first attached submit.
    ///
    /// # Parameters
    ///
    /// `state`     - state of the first submit.
    /// `sid`       - id of the first submit.
    ///
    pub fn new(sid: SubmitId, state: State<R>) -> Self {
        use std::iter::once;
        Link {
            state,
            queues: once((sid.queue().index(), LinkQueueState::new(sid, state))).collect(),
            family: sid.family(),
        }
    }

    /// Get queue family that owns the resource at the link.
    /// All associated submits must be from the same queue family.
    pub fn family(&self) -> QueueFamilyId {
        self.family
    }

    ///
    pub fn access(&self) -> R::Access {
        self.state.access
    }

    ///
    pub fn layout(&self) -> R::Layout {
        self.state.layout
    }

    ///
    pub fn stages(&self) -> PipelineStage {
        self.state.stages
    }

    /// Check if the link is associated with only one submit.
    pub fn exclusive(&self) -> bool {
        let mut values = self.queues.values();
        let queue = values.next().unwrap();
        values.next().is_none() && queue.first() == queue.last()
    }

    /// Check of the given states and submit are compatible with link.
    /// If those are compatible then this submit can be associated with the link.
    pub fn compatible(&self, sid: SubmitId, state: State<R>) -> bool {
        // If queue the same and states are compatible.
        // And there is no later submit on the queue.
        self.family == sid.family() && self.state.compatible(state)
            && self.queues
                .get(&sid.queue().index())
                .map_or(true, |state| state.last() + 1 == sid.index())
    }

    /// Insert submit with specified state to the link.
    /// It must be compatible.
    /// Associating submit with the link will allow the submit
    /// to be executed in parallel with other submits associated with this link.
    /// Unless other chains disallow.
    ///
    /// # Panics
    ///
    /// This function will panic if `state` and `sid` are not compatible.
    /// E.g. `Link::compatible` didn't returned `true` for the arguments.
    ///
    pub fn insert_submit(&mut self, sid: SubmitId, state: State<R>) {
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
    }

    /// Collect first submits from all queues.
    /// Can be used to calculate wait-factor before inserting new submit.
    pub fn heads(&self) -> Vec<SubmitId> {
        self.queues
            .iter()
            .map(|(&index, queue)| SubmitId::new(QueueId::new(self.family, index), queue.first()))
            .collect()
    }

    /// Collect last submits from all queues.
    /// Can be used to calculate wait-factor before inserting new submit.
    pub fn tails(&self) -> Vec<SubmitId> {
        self.queues
            .iter()
            .map(|(&index, queue)| SubmitId::new(QueueId::new(self.family, index), queue.last()))
            .collect()
    }

    /// Check if ownership transfer is required between those links.
    pub fn transfer(&self, next: &Self) -> bool {
        self.family != next.family
    }

    ///
    pub fn queue(&self, qid: QueueId) -> Option<&LinkQueueState<R>> {
        assert_eq!(self.family, qid.family());
        self.queues.get(&qid.index())
    }

    ///
    pub fn queues(&self) -> QueuesIter<R> {
        QueuesIter {
            family: self.family,
            iter: self.queues.iter(),
        }
    }

    ///
    pub fn queue_state(&self, qid: QueueId) -> State<R> {
        let queue = self.queue(qid).unwrap();
        State {
            access: queue.access,
            layout: self.state.layout,
            stages: queue.stages,
        }
    }
}

impl<R> Index<QueueId> for Link<R>
where
    R: Resource,
{
    type Output = LinkQueueState<R>;

    fn index(&self, qid: QueueId) -> &LinkQueueState<R> {
        self.queue(qid).unwrap()
    }
}

pub struct QueuesIter<'a, R: Resource + 'a> {
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
