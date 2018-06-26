use std::iter::Enumerate;
use std::slice::Iter as SliceIter;

use hal::pso::PipelineStage;
use hal::queue::QueueFamilyId;

use resource::{Resource, State};
use schedule::{QueueId, SubmissionId};

/// State of the link associated with queue.
/// Contains submissions range, combined access and stages bits by submissions from the range.
#[derive(Clone, Debug)]
pub(crate) struct LinkQueueState<R: Resource> {
    pub first: usize,
    pub last: usize,
    pub access: R::Access,
    pub stages: PipelineStage,
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
}

/// This type defines what states resource are at some point in time when commands recorded into
/// corresponding submissions are executed.
/// Those commands doesn't required to perform actions with all access types declared by the link.
/// But performing actions with access types not declared by the link is prohibited.
#[derive(Clone, Debug)]
pub struct Link<R: Resource> {
    usage: R::Usage,
    state: State<R>,
    queue_count: usize,
    queues: Vec<Option<LinkQueueState<R>>>,
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
        let mut link = Link {
            state,
            queue_count: 1,
            queues: Vec::new(),
            family: sid.family(),
            usage,
        };
        link.ensure_queue(sid.queue().index());
        link.queues[sid.queue().index()] = Some(LinkQueueState::new(sid, state));
        link
    }

    fn ensure_queue(&mut self, index: usize) {
        if index >= self.queues.len() {
            let reserve = index - self.queues.len() + 1;
            self.queues.reserve(reserve);
            while index >= self.queues.len() {
                self.queues.push(None);
            }
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

    /// Check if the link is associated with only one queue.
    pub fn single_queue(&self) -> bool {
        self.queue_count == 1
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
        self.ensure_queue(sid.queue().index());

        let state = self.state.merge(state);
        match &mut self.queues[sid.queue().index()] {
            &mut Some(ref mut queue) => {
                queue.push(sid, state);
            }
            slot @ &mut None => {
                self.queue_count += 1;
                *slot = Some(LinkQueueState::new(sid, state));
            }
        }
        self.state = state;
        self.usage |= usage;
    }

    /// Check if ownership transfer is required between those links.
    pub fn transfer(&self, next: &Self) -> bool {
        self.family != next.family
    }

    /// Get iterator over queue
    pub(crate) fn queues(&self) -> QueuesIter<R> {
        QueuesIter {
            family: self.family,
            iter: self.queues.iter().enumerate(),
        }
    }

    ///
    pub(crate) fn queue_state(&self, qid: QueueId) -> State<R> {
        let queue = self.queue(qid);
        State {
            access: queue.access,
            stages: queue.stages,
            ..self.state
        }
    }

    pub(crate) fn queue(&self, qid: QueueId) -> &LinkQueueState<R> {
        debug_assert_eq!(self.family, qid.family());
        self.queues[qid.index()].as_ref().unwrap()
    }
}

pub(crate) struct QueuesIter<'a, R: Resource + 'a> {
    family: QueueFamilyId,
    iter: Enumerate<SliceIter<'a, Option<LinkQueueState<R>>>>,
}

impl<'a, R: Resource + 'a> Iterator for QueuesIter<'a, R> {
    type Item = (QueueId, &'a LinkQueueState<R>);

    fn next(&mut self) -> Option<(QueueId, &'a LinkQueueState<R>)> {
        while let Some((index, state)) = self.iter.next() {
            if let &Some(ref queue) = state {
                return Some((QueueId::new(self.family, index), queue));
            }
        }
        None
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (0, self.iter.size_hint().1)
    }
}
