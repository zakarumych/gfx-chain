#![allow(missing_docs)]

use std::collections::HashMap;
use std::ops::{Range, RangeFrom, RangeTo};

use hal::pso::PipelineStage;

use Pick;
use chain::{BufferChains, Chain, ImageChains, Link};
use families::{Families, QueueId, Submit, SubmitId};
use resource::{Access, Buffer, Id, Image, Resource, State};
use schedule::Schedule;

pub enum Side {
    Acquire,
    Release,
}

pub struct Semaphore {
    pub sid: SubmitId,
    pub stages: PipelineStage,
    pub side: Side,
}

pub struct Barrier<R: Resource> {
    pub queues: Option<Range<QueueId>>,
    pub states: Range<State<R>>,
}

impl<R> Barrier<R>
where
    R: Resource,
{
    fn new(states: Range<State<R>>) -> Self {
        Barrier {
            queues: None,
            states,
        }
    }

    fn transfer(queues: Range<QueueId>, states: Range<State<R>>) -> Self {
        Barrier {
            queues: Some(queues),
            states,
        }
    }

    fn acquire(
        queues: Range<QueueId>,
        left: RangeFrom<R::Layout>,
        right: RangeTo<State<R>>,
    ) -> Self {
        Self::transfer(
            queues,
            State {
                access: R::Access::none(),
                layout: left.start,
                stages: PipelineStage::empty(),
            }..right.end,
        )
    }

    fn release(
        queues: Range<QueueId>,
        left: RangeFrom<State<R>>,
        right: RangeTo<R::Layout>,
    ) -> Self {
        Self::transfer(
            queues,
            left.start..State {
                access: R::Access::none(),
                layout: right.end,
                stages: PipelineStage::empty(),
            },
        )
    }
}

pub type Barriers<R: Resource> = HashMap<Id<R>, Barrier<R>>;
pub type BufferBarriers = Barriers<Buffer>;
pub type ImageBarriers = Barriers<Image>;

pub struct Guard {
    wait: Vec<Semaphore>,
    buffers: BufferBarriers,
    images: ImageBarriers,
    signal: Vec<Semaphore>,
}

impl Guard {
    fn new() -> Self {
        Guard {
            wait: Vec::new(),
            buffers: HashMap::new(),
            images: HashMap::new(),
            signal: Vec::new(),
        }
    }
}

impl Pick<Image> for Guard {
    type Target = ImageBarriers;

    fn pick(&self) -> &ImageBarriers {
        &self.images
    }
    fn pick_mut(&mut self) -> &mut ImageBarriers {
        &mut self.images
    }
}

impl Pick<Buffer> for Guard {
    type Target = BufferBarriers;

    fn pick(&self) -> &BufferBarriers {
        &self.buffers
    }
    fn pick_mut(&mut self) -> &mut BufferBarriers {
        &mut self.buffers
    }
}

pub struct Sync {
    pub acquire: Guard,
    pub release: Guard,
}

impl Sync {
    fn new() -> Self {
        Sync {
            acquire: Guard::new(),
            release: Guard::new(),
        }
    }
}

fn latest<R>(link: &Link<R>, families: &Families) -> SubmitId
where
    R: Resource,
{
    let (_, sid) = link.queues()
        .map(|(qid, queue)| {
            let sid = SubmitId::new(qid, queue.last());
            (families[sid].wait_factor, sid)
        })
        .max()
        .unwrap();
    sid
}

fn earliest<R>(link: &Link<R>, families: &Families) -> SubmitId
where
    R: Resource,
{
    let (_, sid) = link.queues()
        .map(|(qid, queue)| {
            let sid = SubmitId::new(qid, queue.first());
            (families[sid].wait_factor, sid)
        })
        .min()
        .unwrap();
    sid
}

fn sync_submit_chain<R>(
    sid: SubmitId,
    _submit: &Submit,
    link_index: usize,
    id: Id<R>,
    chain: &Chain<R>,
    families: &Families,
    sync: &mut Sync,
) where
    R: Resource,
    Guard: Pick<R, Target = Barriers<R>>,
{
    let ref this = chain.links()[link_index];
    assert_eq!(this.family(), sid.family());
    let ref queue = this[sid.queue()];
    assert!(queue.first() <= sid.index());
    assert!(queue.last() >= sid.index());

    let this_state = this.queue_state(sid.queue());

    if queue.first() == sid.index() && link_index > 0 {
        // First of queue. Sync with prev.
        let ref prev = chain.links()[link_index - 1];
        if prev.family() != this.family() {
            // Transfer ownership.

            // Find earliest submit from this chain.
            // It will acquire ownership.
            let this_earliest = earliest(this, families);
            if this_earliest == sid {
                // Find latest submit from prev chain.
                // It will release ownership.
                let prev_latest = latest(prev, families);

                // Wait for release.
                sync.acquire.wait.push(Semaphore {
                    sid: prev_latest,
                    stages: this_state.stages,
                    side: Side::Release,
                });

                // Acquire ownership.
                sync.acquire.pick_mut().insert(
                    id,
                    Barrier::acquire(
                        prev_latest.queue()..sid.queue(),
                        prev.layout()..,
                        ..this_state,
                    ),
                );

                // Signal to other queues in this link.
                for (qid, queue) in this.queues().filter(|&(qid, _)| qid != sid.queue()) {
                    sync.acquire.signal.push(Semaphore {
                        sid: SubmitId::new(qid, queue.first()),
                        stages: PipelineStage::BOTTOM_OF_PIPE,
                        side: Side::Acquire,
                    })
                }
            } else {
                // This is not the earliest.
                // Wait for earliest.
                sync.acquire.wait.push(Semaphore {
                    sid: this_earliest,
                    stages: this_state.stages,
                    side: Side::Acquire,
                });
            }
        } else {
            // Same family.
            for tail in prev.tails() {
                if tail.queue() != sid.queue() {
                    // Wait for tails on other queues.
                    sync.acquire.wait.push(Semaphore {
                        sid: tail,
                        stages: this_state.stages,
                        side: Side::Release,
                    });
                } else if !prev.exclusive() {
                    // Insert barrier here.
                    // Prev won't insert as it isn't exclusive.
                    assert!(this.exclusive());
                    sync.acquire
                        .pick_mut()
                        .insert(id, Barrier::new(prev.queue_state(tail.queue())..this_state));
                }
            }
        }
    }

    if queue.last() == sid.index() && link_index + 1 < chain.links().len() {
        // Sync with next.
        let ref next = chain.links()[link_index + 1];
        if next.family() != this.family() {
            // Transfer ownership.

            // Find latest submit from this chain.
            // It will release ownership.
            let this_latest = latest(this, families);
            if this_latest == sid {
                // Wait for other queues in this link.
                for (qid, queue) in this.queues().filter(|&(qid, _)| qid != sid.queue()) {
                    sync.release.wait.push(Semaphore {
                        sid: SubmitId::new(qid, queue.last()),
                        stages: PipelineStage::TOP_OF_PIPE,
                        side: Side::Release,
                    })
                }

                // Find earliest submit from next chain.
                // It will acquire ownership.
                let next_earliest = earliest(next, families);

                // Release ownership.
                sync.release.pick_mut().insert(
                    id,
                    Barrier::release(
                        sid.queue()..next_earliest.queue(),
                        this_state..,
                        ..next.layout(),
                    ),
                );

                // Signal to acquire.
                sync.release.signal.push(Semaphore {
                    sid: next_earliest,
                    stages: this_state.stages,
                    side: Side::Acquire,
                });
            } else {
                // This is not the latest.
                // Signal to latest.
                sync.release.signal.push(Semaphore {
                    sid: this_latest,
                    stages: this_state.stages,
                    side: Side::Release,
                });
            }
        } else {
            // Same family.
            for head in next.heads() {
                if head.queue() != sid.queue() {
                    // Signal to heads on other queues.
                    sync.release.signal.push(Semaphore {
                        sid: head,
                        stages: this_state.stages,
                        side: Side::Acquire,
                    });
                } else if this.exclusive() {
                    // Insert barrier here.
                    // Next won't insert as this is exclusive.
                    assert!(!next.exclusive());
                    sync.release
                        .pick_mut()
                        .insert(id, Barrier::new(next.queue_state(head.queue())..this_state));
                }
            }
        }
    }
}

fn sync_submit(
    sid: SubmitId,
    submit: &Submit,
    buffers: &BufferChains,
    images: &ImageChains,
    families: &Families,
) -> Sync {
    let mut sync = Sync::new();
    for (&id, &index) in submit.buffers.iter() {
        let ref chain = buffers[&id];
        sync_submit_chain(sid, submit, index, id, chain, families, &mut sync);
    }

    for (&id, &index) in submit.images.iter() {
        let ref chain = images[&id];
        sync_submit_chain(sid, submit, index, id, chain, families, &mut sync);
    }

    // TODO: Cleanup.
    // Remove redundant semaphores.

    sync
}

/// Synchronize schedule.
pub fn sync(schedule: &Schedule) -> HashMap<SubmitId, Sync> {
    let ref families = schedule.families;
    let ref buffers = schedule.buffers;
    let ref images = schedule.images;

    families
        .iter()
        .flat_map(|family| family.iter())
        .flat_map(|queue| queue.iter())
        .map(|(sid, submit)| (sid, sync_submit(sid, submit, buffers, images, families)))
        .collect()
}
