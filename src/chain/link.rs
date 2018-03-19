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
    Passes(HashMap<usize, LinkQueueState<R>>),
    Acquire { queue: usize, submit: usize },
    Release { queue: usize, submit: usize },
}

/// This type defines what states resource are at some point in time when commands recorded into
/// contained submissions are executed.
/// Those commands doesn't required to perform actions with all access types declared by the link.
/// Performing actions with access types not declared by the link is prohibited.
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
            queues: LinkQueues::Passes(
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
    pub fn new_acquire(state: State<R>, sid: SubmitId) -> Self {
        Link {
            state,
            queues: LinkQueues::Acquire {
                queue: sid.queue().index(),
                submit: sid.index(),
            },
            family: sid.family(),
        }
    }

    /// Create new link
    pub fn new_release(state: State<R>, sid: SubmitId) -> Self {
        Link {
            state,
            queues: LinkQueues::Release {
                queue: sid.queue().index(),
                submit: sid.index(),
            },
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

    /// Get queue state
    pub fn queue_state(&self, qid: QueueId) -> State<R> {
        assert_eq!(self.family, qid.family());
        match self.queues {
            LinkQueues::Acquire { queue, .. } | LinkQueues::Release { queue, .. } => {
                assert_eq!(queue, qid.index());
                self.state
            }
            LinkQueues::Passes(ref map) => {
                let queue = map.get(&qid.index()).unwrap();
                assert_eq!(self.state.access, self.state.access | queue.access);
                assert_eq!(self.state.stages, self.state.stages | queue.stages);
                State {
                    access: queue.access,
                    stages: queue.stages,
                    layout: self.state.layout,
                }
            }
        }
    }

    /// Check of the given states and submit are compatible with link.
    pub fn compatible(&self, state: State<R>, sid: SubmitId) -> bool {
        // If queue the same and states are compatible.
        // And there is no later submit on the queue.
        self.family == sid.family() && self.state.compatible(state) && match self.queues {
            LinkQueues::Passes(ref map) => map.get(&sid.queue().index())
                .map_or(true, |state| state.submits.end == sid.index()),
            _ => false,
        }
    }

    /// Push submit with specified state to the link.
    /// It must be compatible.
    pub fn insert_submit(&mut self, state: State<R>, sid: SubmitId) {
        assert_eq!(self.family, sid.family());
        let state = self.state.merge(state);
        match self.queues {
            LinkQueues::Passes(ref mut map) => match map.entry(sid.queue().index()) {
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
            _ => panic!("Inserting submits into acquire and release links isn't allowed"),
        }
        self.state = state;
    }

    /// Collect tails of submits from all queues.
    /// Can be used to calculate wait-factor before inserting new submit.
    pub fn tails(&self) -> Vec<SubmitId> {
        match self.queues {
            LinkQueues::Passes(ref map) => map.iter()
                .map(|(&index, queue)| {
                    SubmitId::new(QueueId::new(self.family, index), queue.submits.end - 1)
                })
                .collect(),
            LinkQueues::Acquire { queue, submit } | LinkQueues::Release { queue, submit } => {
                vec![SubmitId::new(QueueId::new(self.family, queue), submit)]
            }
        }
    }

    /// Check if ownership transfer is required
    pub fn transfer(&self, next: &Self) -> bool {
        self.family != next.family
    }

    /// Get all semaphores required to synchronize those links.
    pub fn semaphores(&self, next: &Self) -> Vec<Semaphore> {
        assert!(self.state.exclusive() || next.state.exclusive());
        match (&self.queues, &next.queues) {
            (&LinkQueues::Passes(ref left), &LinkQueues::Passes(ref right)) => {
                assert_eq!(self.family, next.family);
                assert!(left.len() == 1 || right.len() == 1);
                left.iter()
                    .flat_map(|(&lqid, lqueue)| {
                        let lsid =
                            SubmitId::new(QueueId::new(self.family, lqid), lqueue.submits.end - 1);
                        right.iter().filter_map(move |(&rqid, rqueue)| {
                            let rsid = SubmitId::new(
                                QueueId::new(next.family, rqid),
                                rqueue.submits.start,
                            );
                            if lqid != rqid {
                                Some(Semaphore {
                                    submits: lsid..rsid,
                                    stages: lqueue.stages..rqueue.stages,
                                })
                            } else {
                                None
                            }
                        })
                    })
                    .collect()
            }
            (
                &LinkQueues::Passes(ref left),
                &LinkQueues::Release {
                    queue: rqid,
                    submit: rsid,
                },
            ) => {
                assert_eq!(self.family, next.family);
                let rsid = SubmitId::new(QueueId::new(next.family, rqid), rsid);
                left.iter()
                    .filter_map(|(&lqid, lqueue)| {
                        let lsid =
                            SubmitId::new(QueueId::new(self.family, lqid), lqueue.submits.end - 1);
                        if lqid != rqid {
                            Some(Semaphore {
                                submits: lsid..rsid,
                                stages: lqueue.stages..next.state.stages,
                            })
                        } else {
                            None
                        }
                    })
                    .collect()
            }
            (
                &LinkQueues::Acquire {
                    queue: lqid,
                    submit: lsid,
                },
                &LinkQueues::Passes(ref right),
            ) => {
                assert_eq!(self.family, next.family);
                let lsid = SubmitId::new(QueueId::new(self.family, lqid), lsid);
                right
                    .iter()
                    .filter_map(|(&rqid, rqueue)| {
                        let rsid =
                            SubmitId::new(QueueId::new(next.family, rqid), rqueue.submits.start);
                        if lqid != rqid {
                            Some(Semaphore {
                                submits: lsid..rsid,
                                stages: self.state.stages..rqueue.stages,
                            })
                        } else {
                            None
                        }
                    })
                    .collect()
            }
            (
                &LinkQueues::Release {
                    queue: lqid,
                    submit: lsid,
                },
                &LinkQueues::Acquire {
                    queue: rqid,
                    submit: rsid,
                },
            ) => {
                assert_ne!(self.family, next.family);
                let lsid = SubmitId::new(QueueId::new(self.family, lqid), lsid);
                let rsid = SubmitId::new(QueueId::new(next.family, rqid), rsid);
                vec![
                    Semaphore {
                        submits: lsid..rsid,
                        stages: self.state.stages..next.state.stages,
                    },
                ]
            }
            _ => panic!("Invalid chain"),
        }
    }

    /// Get all barriers required to synchronize those links.
    pub fn barriers(&self, next: &Self) -> Vec<Barrier<R>> {
        match (&self.queues, &next.queues) {
            (&LinkQueues::Passes(ref left), &LinkQueues::Passes(ref right)) => left.iter()
                .flat_map(|(&lqid, lqueue)| {
                    let lsid = SubmitId::new(QueueId::new(self.family, lqid), lqueue.submits.start);
                    right.iter().flat_map(move |(&rqid, rqueue)| {
                        let rsid =
                            SubmitId::new(QueueId::new(next.family, rqid), rqueue.submits.start);
                        if lsid.queue() == rsid.queue() {
                            Some(Barrier {
                                submits: lsid..rsid,
                                accesses: lqueue.access..rqueue.access,
                                layouts: self.state.layout..next.state.layout,
                                stages: lqueue.stages..rqueue.stages,
                            })
                        } else {
                            None
                        }
                    })
                })
                .collect(),
            (
                &LinkQueues::Passes(ref left),
                &LinkQueues::Release {
                    queue: rqid,
                    submit: rsid,
                },
            ) => {
                assert_eq!(self.family, next.family);
                let rsid = SubmitId::new(QueueId::new(next.family, rqid), rsid);
                left.iter()
                    .filter_map(|(&lqid, lqueue)| {
                        let lsid =
                            SubmitId::new(QueueId::new(self.family, lqid), lqueue.submits.end - 1);
                        if lqid == rqid {
                            Some(Barrier {
                                submits: lsid..rsid,
                                accesses: lqueue.access..next.state.access,
                                layouts: self.state.layout..next.state.layout,
                                stages: lqueue.stages..next.state.stages,
                            })
                        } else {
                            None
                        }
                    })
                    .collect()
            }
            (
                &LinkQueues::Acquire {
                    queue: lqid,
                    submit: lsid,
                },
                &LinkQueues::Passes(ref right),
            ) => {
                assert_eq!(self.family, next.family);
                let lsid = SubmitId::new(QueueId::new(self.family, lqid), lsid);
                right
                    .iter()
                    .filter_map(|(&rqid, rqueue)| {
                        let rsid =
                            SubmitId::new(QueueId::new(next.family, rqid), rqueue.submits.start);
                        if lqid == rqid {
                            Some(Barrier {
                                submits: lsid..rsid,
                                accesses: self.state.access..next.state.access,
                                layouts: self.state.layout..next.state.layout,
                                stages: self.state.stages..rqueue.stages,
                            })
                        } else {
                            None
                        }
                    })
                    .collect()
            }
            (
                &LinkQueues::Release {
                    queue: lqid,
                    submit: lsid,
                },
                &LinkQueues::Acquire {
                    queue: rqid,
                    submit: rsid,
                },
            ) => {
                assert_ne!(self.family, next.family);
                let lsid = SubmitId::new(QueueId::new(self.family, lqid), lsid);
                let rsid = SubmitId::new(QueueId::new(next.family, rqid), rsid);

                let barrier = Barrier {
                    submits: lsid..rsid,
                    accesses: self.state.access..next.state.access,
                    layouts: self.state.layout..next.state.layout,
                    stages: self.state.stages..next.state.stages,
                };
                vec![barrier]
            }
            _ => panic!("Invalid chain"),
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
