use std::collections::hash_map::{Entry, HashMap};
use std::ops::Range;

use hal::pso::PipelineStage;
use hal::queue::QueueFamilyId;

use resource::{Resource, State};
use families::{QueueId, SubmitId};

#[derive(Clone, Debug)]
pub struct LinkQueueState<R: Resource> {
    pub(super) access: R::Access,
    pub(super) stages: PipelineStage,
    pub(super) submits: Range<usize>,
}

/// This type defines what states resource are at some point in time when commands recorded into
/// contained submissions are executed.
/// Those commands doesn't required to perform actions with all access types declared by the link.
/// Performing actions with access types not declared by the link is prohibited.
#[derive(Clone, Debug)]
pub struct Link<R: Resource> {
    pub(super) state: State<R>,
    pub(super) queues: HashMap<usize, LinkQueueState<R>>,
    pub(super) family: QueueFamilyId,
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
    pub fn new(state: State<R>, sid: SubmitId) -> Self {
        use std::iter::once;
        Link {
            state,
            queues: once((
                sid.queue().index(),
                LinkQueueState {
                    access: state.access,
                    stages: state.stages,
                    submits: sid.index()..sid.index() + 1,
                },
            )).collect(),
            family: sid.family(),
        }
    }

    /// Get queue family that owns the resource at the link.
    /// All associated submits must be from the same queue family.
    pub fn family(&self) -> QueueFamilyId {
        self.family
    }

    /// Get total state of the resource.
    /// Total state is combination of all access types
    /// performed by the associated submits,
    /// all stages at which resource is accessed
    /// and layout that supports all operations performed by all submits.
    pub fn total_state(&self) -> State<R> {
        self.state
    }

    /// Get queue state.
    /// This state contains only access types and stages
    /// for submits from specified queue.
    ///
    /// # Panics
    ///
    /// This function will panic if this link has no associated
    /// submits from specified queue.
    ///
    pub fn queue_state(&self, qid: QueueId) -> State<R> {
        assert_eq!(self.family, qid.family());
        let queue = self.queues.get(&qid.index()).unwrap();
        assert_eq!(self.state.access, self.state.access | queue.access);
        assert_eq!(self.state.stages, self.state.stages | queue.stages);
        State {
            access: queue.access,
            stages: queue.stages,
            layout: self.state.layout,
        }
    }

    /// Check if the link is associated only with specified submit.
    ///
    /// # Panic
    ///
    /// The function will panic if link doesn't associated with specified submit.
    ///
    pub fn exclusive(&self, sid: SubmitId) -> bool {
        assert_eq!(self.family, sid.family());
        let range = self.queues[&sid.queue().index()].submits.clone();
        assert!(range.start <= sid.index() && range.end > sid.index());
        range.start + 1 == range.end
    }

    /// Check of the given states and submit are compatible with link.
    /// If those are compatible then this submit can be associated with the link.
    pub fn compatible(&self, state: State<R>, sid: SubmitId) -> bool {
        // If queue the same and states are compatible.
        // And there is no later submit on the queue.
        self.family == sid.family() && self.state.compatible(state)
            && self.queues
                .get(&sid.queue().index())
                .map_or(true, |state| state.submits.end == sid.index())
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
    pub fn insert_submit(&mut self, state: State<R>, sid: SubmitId) {
        assert_eq!(self.family, sid.family());
        let state = self.state.merge(state);
        match self.queues.entry(sid.queue().index()) {
            Entry::Occupied(mut occupied) => {
                assert_eq!(occupied.get_mut().submits.end, sid.index());
                occupied.get_mut().access |= state.access;
                occupied.get_mut().stages |= state.stages;
                occupied.get_mut().submits.end = sid.index() + 1;
            }
            Entry::Vacant(vacant) => {
                vacant.insert(LinkQueueState {
                    access: state.access,
                    stages: state.stages,
                    submits: sid.index()..sid.index() + 1,
                });
            }
        };
        self.state = state;
    }

    /// Collect last submits from all queues.
    /// Can be used to calculate wait-factor before inserting new submit.
    pub fn tails(&self) -> Vec<SubmitId> {
        self.queues
            .iter()
            .map(|(&index, queue)| {
                SubmitId::new(QueueId::new(self.family, index), queue.submits.end - 1)
            })
            .collect()
    }

    /// Check if ownership transfer is required between those links.
    pub fn transfer(&self, next: &Self) -> bool {
        self.family != next.family
    }
}
