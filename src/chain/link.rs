use std::collections::hash_map::{Entry, HashMap, Iter as HashMapIter};
use std::option::IntoIter as OptionIter;
use std::ops::Range;

use hal::pso::PipelineStage;
use hal::queue::QueueFamilyId;

use resource::{Resource, State};
use families::{QueueId, SubmitId};

#[derive(Clone, Debug)]
pub struct LinkQueueState<R: Resource> {
    pub(crate) access: R::Access,
    pub(crate) stages: PipelineStage,
    pub(crate) submits: Range<usize>,
}

#[derive(Clone, Debug)]
enum LinkQueues<R: Resource> {
    MultiQueue(HashMap<usize, LinkQueueState<R>>),
    SingleQueue(usize, LinkQueueState<R>),
}

impl<R> LinkQueues<R>
where
    R: Resource,
{
    fn iter(&self) -> LinkQueuesIter<R> {
        match *self {
            LinkQueues::MultiQueue(ref map) => LinkQueuesIter::MultiQueue(map.iter()),
            LinkQueues::SingleQueue(ref index, ref queue) => {
                LinkQueuesIter::SingleQueue(Some((index, queue)).into_iter())
            }
        }
    }
}

enum LinkQueuesIter<'a, R: Resource + 'a> {
    MultiQueue(HashMapIter<'a, usize, LinkQueueState<R>>),
    SingleQueue(OptionIter<(&'a usize, &'a LinkQueueState<R>)>),
}

impl<'a, R> Iterator for LinkQueuesIter<'a, R>
where
    R: Resource + 'a,
{
    type Item = (&'a usize, &'a LinkQueueState<R>);

    fn next(&mut self) -> Option<(&'a usize, &'a LinkQueueState<R>)> {
        match *self {
            LinkQueuesIter::SingleQueue(ref mut iter) => iter.next(),
            LinkQueuesIter::MultiQueue(ref mut iter) => iter.next(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct Link<R: Resource> {
    state: State<R>,
    queues: LinkQueues<R>,
    family: QueueFamilyId,
}

impl<R> Link<R>
where
    R: Resource,
{
    /// Create new link
    pub fn new(state: State<R>, sid: SubmitId) -> Self {
        use std::iter::once;
        Link {
            state,
            queues: LinkQueues::MultiQueue(
                once((
                    sid.queue().index(),
                    LinkQueueState {
                        access: state.access,
                        stages: state.stages,
                        submits: sid.index()..sid.index() + 1,
                    },
                )).collect(),
            ),
            family: sid.family(),
        }
    }

    /// Create new link
    pub fn new_single_queue(state: State<R>, sid: SubmitId) -> Self {
        Link {
            state,
            queues: LinkQueues::SingleQueue(
                sid.queue().index(),
                LinkQueueState {
                    access: state.access,
                    stages: state.stages,
                    submits: sid.index()..sid.index() + 1,
                },
            ),
            family: sid.family(),
        }
    }

    /// Get family that owns the resource at the link.
    pub fn family(&self) -> QueueFamilyId {
        self.family
    }

    /// Get total state of the resource
    pub fn total_state(&self) -> State<R> {
        self.state
    }

    /// Get total state of the resource
    pub fn queue_state(&self, qid: QueueId) -> State<R> {
        let queue = self.queue(qid);
        assert_eq!(self.state.access, self.state.access | queue.access);
        assert_eq!(self.state.stages, self.state.stages | queue.stages);
        State {
            access: queue.access,
            stages: queue.stages,
            layout: self.state.layout,
        }
    }

    /// Get queue state
    pub fn queue(&self, qid: QueueId) -> &LinkQueueState<R> {
        assert_eq!(self.family, qid.family());
        match self.queues {
            LinkQueues::SingleQueue(index, ref queue) => {
                assert_eq!(index, qid.index());
                queue
            }
            LinkQueues::MultiQueue(ref map) => map.get(&qid.index()).unwrap(),
        }
    }

    /// Convert to single-queue link.
    /// This convert is irreversible.
    pub fn make_single_queue(&mut self) -> bool {
        let (index, queue) = match self.queues {
            LinkQueues::SingleQueue(..) => return true,
            LinkQueues::MultiQueue(ref mut map) => {
                assert!(!map.is_empty());
                if map.len() != 1 {
                    return false;
                } else {
                    map.drain().next().unwrap()
                }
            }
        };
        self.queues = LinkQueues::SingleQueue(index, queue);
        true
    }

    /// Check of the given states and submit are compatible with link.
    pub fn compatible(&self, state: State<R>, sid: SubmitId) -> bool {
        // If queue the same and states are compatible.
        // And there is no later submit on the queue.
        self.family == sid.family() && self.state.compatible(state) && match self.queues {
            LinkQueues::MultiQueue(ref map) => map.get(&sid.queue().index())
                .map_or(true, |state| state.submits.end == sid.index()),
            LinkQueues::SingleQueue(index, ref queue) => {
                index == sid.queue().index() && queue.submits.end == sid.index()
            }
        }
    }

    /// Push submit with specified state to the link.
    /// It must be compatible.
    pub fn push(&mut self, state: State<R>, sid: SubmitId) {
        assert_eq!(self.family, sid.family());
        let state = self.state.merge(state);
        match self.queues {
            LinkQueues::MultiQueue(ref mut map) => match map.entry(sid.queue().index()) {
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
            },
            LinkQueues::SingleQueue(index, ref mut queue) => {
                assert_eq!(index, sid.queue().index());
                assert_eq!(queue.submits.end, sid.index());
                queue.access |= state.access;
                queue.stages |= state.stages;
                queue.submits.end = sid.index() + 1;
            }
        }
        self.state = state;
    }

    /// Collect tails of submits from all queues.
    /// Can be used to calculate wait-factor before inserting new submit.
    pub fn tails(&self) -> Vec<SubmitId> {
        self.queues
            .iter()
            .map(|(&index, queue)| {
                SubmitId::new(QueueId::new(self.family, index), queue.submits.end - 1)
            })
            .collect()
    }

    /// Check if ownership transfer is required
    pub fn transfer(&self, next: &Self) -> bool {
        self.family != next.family
    }

    /// Get all semaphores required to synchronize those links.
    pub fn semaphores(&self, next: &Self) -> Vec<Semaphore> {
        self.queues
            .iter()
            .flat_map(|(&lqid, lqueue)| {
                let lqid = QueueId::new(self.family, lqid);
                next.queues.iter().filter_map(move |(&rqid, rqueue)| {
                    let rqid = QueueId::new(next.family, rqid);
                    if lqid != rqid {
                        Some(Semaphore {
                            submits: SubmitId::new(lqid, lqueue.submits.end - 1)
                                ..SubmitId::new(rqid, rqueue.submits.start),
                            stages: lqueue.stages..rqueue.stages,
                        })
                    } else {
                        None
                    }
                })
            })
            .collect()
    }

    /// Get all barriers required to synchronize those links.
    pub fn barriers(&self, next: &Self) -> Vec<Barrier<R>> {
        if self.family != next.family {
            match (&self.queues, &next.queues) {
                (&LinkQueues::MultiQueue(..), _) => panic!("Must be single-queue"),
                (_, &LinkQueues::MultiQueue(..)) => panic!("Must be single-queue"),
                (
                    &LinkQueues::SingleQueue(lqid, ref lqueue),
                    &LinkQueues::SingleQueue(rqid, ref rqueue),
                ) => {
                    let lqid = QueueId::new(self.family, lqid);
                    let rqid = QueueId::new(self.family, rqid);

                    assert_eq!(lqueue.access, self.state.access);
                    assert_eq!(rqueue.access, next.state.access);
                    assert_eq!(lqueue.stages, self.state.stages);
                    assert_eq!(rqueue.stages, next.state.stages);

                    let barrier = Barrier {
                        submits: SubmitId::new(lqid, lqueue.submits.end - 1)
                            ..SubmitId::new(rqid, rqueue.submits.start),
                        accesses: self.state.access..next.state.access,
                        layouts: self.state.layout..next.state.layout,
                        stages: self.state.stages..next.state.stages,
                    };
                    vec![barrier]
                }
            }
        } else {
            self.queues
                .iter()
                .flat_map(|(&lqid, lqueue)| {
                    let lqid = QueueId::new(self.family, lqid);
                    next.queues.iter().flat_map(move |(&rqid, rqueue)| {
                        let rqid = QueueId::new(next.family, rqid);
                        if lqid == rqid {
                            Some(Barrier {
                                submits: SubmitId::new(lqid, lqueue.submits.start)
                                    ..SubmitId::new(rqid, rqueue.submits.start),
                                accesses: lqueue.access..rqueue.access,
                                layouts: self.state.layout..next.state.layout,
                                stages: lqueue.stages..rqueue.stages,
                            })
                        } else {
                            None
                        }
                    })
                })
                .collect()
        }
    }
}

pub struct Semaphore {
    pub submits: Range<SubmitId>,
    pub stages: Range<PipelineStage>,
}

pub struct Barrier<R: Resource> {
    pub submits: Range<SubmitId>,
    pub accesses: Range<R::Access>,
    pub layouts: Range<R::Layout>,
    pub stages: Range<PipelineStage>,
}
