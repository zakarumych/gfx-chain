#![allow(missing_docs)]
#![allow(dead_code)]

use std::cmp::Ordering;
use std::collections::HashMap;
use std::ops::{Range, RangeFrom, RangeTo};

use hal::pso::PipelineStage;

use Pick;
use chain::{BufferChains, Chain, ImageChains, Link};
use families::{Families, QueueId, Submit, SubmitId, Submits};
use resource::{Access, Buffer, Id, Image, Resource, State, Uid};
use schedule::Schedule;

fn earlier_stage(stages: PipelineStage) -> PipelineStage {
    PipelineStage::from_bits((stages.bits() - 1) ^ (stages.bits())).unwrap()
}

// fn stage_index(stage: PipelineStage) -> usize {
//     let index = stage.bits().trailing_zeros();
//     assert_eq!(1 << index, stage.bits(), "Only one stage must be set");
//     index as usize
// }

#[derive(Clone, Copy, Debug, PartialOrd, Ord, PartialEq, Eq)]
pub enum Side {
    Acquire,
    Release,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Point {
    sid: SubmitId,
    side: Side,
}

impl PartialOrd<Self> for Point {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        if self.sid.queue() != other.sid.queue() {
            None
        } else {
            Some(Ord::cmp(
                &(self.sid.index(), self.side),
                &(other.sid.index(), other.side),
            ))
        }
    }
}

#[derive(Debug)]
pub struct Signal {
    id: Uid,
    point: Point,
}

impl Signal {
    fn acquire<R>(id: Id<R>, sid: SubmitId) -> Self
    where
        R: Resource,
    {
        Signal {
            id: id.into(),
            point: Point {
                sid,
                side: Side::Acquire,
            },
        }
    }

    fn release<R>(id: Id<R>, sid: SubmitId) -> Self
    where
        R: Resource,
    {
        Signal {
            id: id.into(),
            point: Point {
                sid,
                side: Side::Release,
            },
        }
    }

    fn id(&self) -> Uid {
        self.id
    }

    fn point(&self) -> Point {
        self.point
    }
}

#[derive(Debug)]
pub struct Wait {
    id: Uid,
    point: Point,
    stage: PipelineStage,
}

impl Wait {
    fn acquire<R>(id: Id<R>, sid: SubmitId, stages: PipelineStage) -> Self
    where
        R: Resource,
    {
        Wait {
            id: id.into(),
            point: Point {
                sid,
                side: Side::Acquire,
            },
            stage: earlier_stage(stages),
        }
    }

    fn release<R>(id: Id<R>, sid: SubmitId, stages: PipelineStage) -> Self
    where
        R: Resource,
    {
        Wait {
            id: id.into(),
            point: Point {
                sid,
                side: Side::Release,
            },
            stage: earlier_stage(stages),
        }
    }

    fn id(&self) -> Uid {
        self.id
    }

    fn point(&self) -> Point {
        self.point
    }

    fn stage(&self) -> PipelineStage {
        self.stage
    }
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
    wait: Vec<Wait>,
    buffers: BufferBarriers,
    images: ImageBarriers,
    signal: Vec<Signal>,
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

    fn get(&self, side: Side) -> &Guard {
        match side {
            Side::Acquire => &self.acquire,
            Side::Release => &self.release,
        }
    }

    fn get_mut(&mut self, side: Side) -> &mut Guard {
        match side {
            Side::Acquire => &mut self.acquire,
            Side::Release => &mut self.release,
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
        .max_by_key(|&(wait_factor, sid)| (wait_factor, sid.queue().index()))
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
        .min_by_key(|&(wait_factor, sid)| (wait_factor, sid.queue().index()))
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
                sync.acquire
                    .wait
                    .push(Wait::release(id, prev_latest, this_state.stages));

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
                    sync.acquire
                        .signal
                        .push(Signal::acquire(id, SubmitId::new(qid, queue.first())));
                }
            } else {
                // This is not the earliest.
                // Wait for earliest.
                sync.acquire
                    .wait
                    .push(Wait::acquire(id, this_earliest, this_state.stages));
            }
        } else {
            // Same family.
            for tail in prev.tails() {
                if tail.queue() != sid.queue() {
                    // Wait for tails on other queues.
                    sync.acquire
                        .wait
                        .push(Wait::release(id, tail, this_state.stages));
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
                    sync.release.wait.push(Wait::release(
                        id,
                        SubmitId::new(qid, queue.last()),
                        PipelineStage::TOP_OF_PIPE,
                    ));
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
                sync.release.signal.push(Signal::acquire(id, next_earliest));
            } else {
                // This is not the latest.
                // Signal to latest.
                sync.release.signal.push(Signal::release(id, this_latest));
            }
        } else {
            // Same family.
            for head in next.heads() {
                if head.queue() != sid.queue() {
                    // Signal to heads on other queues.
                    sync.release.signal.push(Signal::acquire(id, head));
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

fn optimize_submit(sid: SubmitId, sync: &mut HashMap<SubmitId, Sync>) {
    use std::mem::replace;

    // Take all waits
    let mut acquire_wait = replace(&mut sync.get_mut(&sid).unwrap().acquire.wait, Vec::new());
    acquire_wait.sort_by_key(|wait| (wait.stage(), wait.point().sid.index()));

    let mut release_wait = replace(&mut sync.get_mut(&sid).unwrap().release.wait, Vec::new());
    release_wait.sort_by_key(|wait| (wait.stage(), wait.point().sid.index()));

    // Collect them. Mark the side.
    let both_side_wait: Vec<_> = acquire_wait
        .into_iter()
        .map(|wait| (Side::Acquire, wait))
        .chain(release_wait.into_iter().map(|wait| (Side::Release, wait)))
        .collect();
    let mut both_side_wait = both_side_wait.into_iter();

    // Collect good waits here.
    let mut result = Vec::new();

    // Iterate from the back.
    while let Some((side, wait)) = both_side_wait.next_back() {
        // Check all earlier waits.
        // This includes all waits from earlier stages.
        // All waits from acquire side are before waits from release side.
        // Also all waits from earlier submits.
        let redundant = both_side_wait
            .as_slice()
            .iter()
            .rev()
            .map(|&(_, ref wait)| wait)
            .chain(
                (0..sid.index())
                    .rev()
                    .filter_map(|index| {
                        let sid = SubmitId::new(sid.queue(), index);
                        sync.get(&sid)
                            .map(|sync| sync.release.wait.iter().chain(sync.acquire.wait.iter()))
                    })
                    .flat_map(|x| x),
            )
            .any(|earlier| {
                // If waits for non-earlier point.
                earlier.point() >= wait.point()
            });

        if redundant {
            // Delete signal as well.
            let ref mut signal = sync.get_mut(&wait.point().sid)
                .unwrap()
                .get_mut(wait.point().side)
                .signal;
            let index = signal
                .iter()
                .position(|signal| signal.point() == Point { sid, side } && signal.id() == wait.id())
                .unwrap();
        } else {
            // Keep it.
            result.push((side, wait));
        }
    }

    // Return all necessary
    for (side, wait) in result {
        sync.get_mut(&sid).unwrap().get_mut(side).wait.push(wait);
    }
}

pub fn optimize(families: &Families, sync: &mut HashMap<SubmitId, Sync>) {
    for queue in families.iter().flat_map(|family| family.iter()) {
        let mut submits = queue.iter();
        while let Some((sid, _)) = submits.next_back() {
            optimize_submit(sid, sync);
        }
    }
}
