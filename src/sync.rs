//!
//! This crates provide functions for find all required synchronizations (barriers and semaphores).
//!

use std::cmp::Ordering;
use std::collections::HashMap;
use std::ops::{Range, RangeFrom, RangeTo};

use hal::pso::PipelineStage;

use Pick;
use chain::{BufferChains, Chain, ImageChains, Link};
use collect::Chains;
use resource::{Access, Buffer, Id, Image, Resource, State, Uid};
use schedule::{QueueId, Schedule, Submit, SubmitId};

fn earlier_stage(stages: PipelineStage) -> PipelineStage {
    PipelineStage::from_bits((stages.bits() - 1) ^ (stages.bits())).unwrap()
}

/// Side of the submit. `Acquire` or `Release`.
#[derive(Clone, Copy, Debug, PartialOrd, Ord, PartialEq, Eq)]
pub enum Side {
    /// Acquire side of the submit.
    /// Synchronization commands from this side must be recorded before main commands of submit.
    Acquire,

    /// Release side of the submit.
    /// Synchronization commands from this side must be recorded after main commands of submit.
    Release,
}

/// Point in execution graph. Target submit and side.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Point {
    /// Id of submit.
    pub sid: SubmitId,

    /// Side of the submit.
    pub side: Side,
}

impl Point {
    fn new(sid: SubmitId, side: Side) -> Self {
        Point { sid, side }
    }
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

/// Semaphore identifier.
/// It allows to distinguish different semaphores to be later replaced in `Signal`s and `Wait`s
/// for references to semaphores (or tokens associated with real semaphores).
#[derive(Debug, PartialEq, Eq)]
pub struct Semaphore {
    id: Uid,
    points: Range<Point>,
}

impl Semaphore {
    fn new<R>(id: Id<R>, points: Range<Point>) -> Self
    where
        R: Resource,
    {
        Semaphore {
            id: id.into(),
            points,
        }
    }

    /// Id of the signal.
    pub fn id(&self) -> Uid {
        self.id
    }

    /// Point for which to signal.
    pub fn points(&self) -> Range<Point> {
        self.points.clone()
    }
}

/// Semaphore signal info.
/// There must be paired wait.
#[derive(Debug, PartialEq, Eq)]
pub struct Signal<S>(S);

impl<S> Signal<S> {
    /// Create signaling for specified point.
    /// At this point `Wait` must be created as well.
    /// `id` and `point` combination must be unique.
    fn new(semaphore: S) -> Self {
        Signal(semaphore)
    }

    /// Get semaphore of the `Signal`.
    pub fn semaphore(&self) -> &S {
        &self.0
    }
}

/// Semaphore wait info.
/// There must be paired signal.
#[derive(Debug, PartialEq, Eq)]
pub struct Wait<S>(S, PipelineStage);

impl<S> Wait<S> {
    /// Create waiting for specified point.
    /// At this point `Signal` must be created as well.
    /// `id` and `point` combination must be unique.
    fn new(semaphore: S, stages: PipelineStage) -> Self {
        Wait(semaphore, earlier_stage(stages))
    }

    /// Get semaphore of the `Wait`.
    pub fn semaphore(&self) -> &S {
        &self.0
    }

    /// Stage at which to wait.
    pub fn stage(&self) -> PipelineStage {
        self.1
    }
}

/// Pipeline barrier info.
pub struct Barrier<R: Resource> {
    /// `Some` queue for ownership transfer. Or `None`
    pub queues: Option<Range<QueueId>>,

    /// Stage transition.
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

/// Map of barriers by resource id.
pub type Barriers<R: Resource> = HashMap<Id<R>, Barrier<R>>;

/// Map of barriers by buffer id.
pub type BufferBarriers = Barriers<Buffer>;

/// Map of barriers by image id.
pub type ImageBarriers = Barriers<Image>;

/// Synchronization for submit at one side.
pub struct Guard<S> {
    /// Points at other queues that must be waited before commands from the submit can be executed.
    pub wait: Vec<Wait<S>>,

    /// Buffer pipeline barriers to be inserted before or after (depends on the side) main commands of the submit.
    pub buffers: BufferBarriers,

    /// Image pipeline barriers to be inserted before or after (depends on the side) main commands of the submit.
    pub images: ImageBarriers,

    /// Points at other queues that can run after barriers above.
    pub signal: Vec<Signal<S>>,
}

impl<S> Guard<S> {
    fn new() -> Self {
        Guard {
            wait: Vec::new(),
            buffers: HashMap::new(),
            images: HashMap::new(),
            signal: Vec::new(),
        }
    }
}

impl<S> Pick<Image> for Guard<S> {
    type Target = ImageBarriers;

    fn pick(&self) -> &ImageBarriers {
        &self.images
    }
    fn pick_mut(&mut self) -> &mut ImageBarriers {
        &mut self.images
    }
}

impl<S> Pick<Buffer> for Guard<S> {
    type Target = BufferBarriers;

    fn pick(&self) -> &BufferBarriers {
        &self.buffers
    }
    fn pick_mut(&mut self) -> &mut BufferBarriers {
        &mut self.buffers
    }
}

/// Both sides of synchronization for submit.
pub struct Sync<S> {
    /// Acquire side of submit synchronization.
    pub acquire: Guard<S>,
    /// Release side of submit synchronization.
    pub release: Guard<S>,
}

impl<S> Sync<S> {
    fn new() -> Self {
        Sync {
            acquire: Guard::new(),
            release: Guard::new(),
        }
    }

    /// Get reference to `Guard` by `Side`.
    pub fn get(&self, side: Side) -> &Guard<S> {
        match side {
            Side::Acquire => &self.acquire,
            Side::Release => &self.release,
        }
    }

    /// Get mutable reference to `Guard` by `Side`.
    pub fn get_mut(&mut self, side: Side) -> &mut Guard<S> {
        match side {
            Side::Acquire => &mut self.acquire,
            Side::Release => &mut self.release,
        }
    }
}

/// Find required synchronization for all submits in `Chains`.
pub fn sync<F, S>(chains: &Chains, new_semaphore: F) -> Schedule<Sync<S>>
where
    F: FnMut() -> S,
{
    let ref schedule = chains.schedule;
    let ref buffers = chains.buffers;
    let ref images = chains.images;

    let mut sync = schedule
        .iter()
        .flat_map(|family| family.iter())
        .flat_map(|queue| queue.iter())
        .map(|(sid, submit)| (sid, sync_submit(sid, submit, buffers, images, schedule)))
        .collect();

    optimize(schedule, &mut sync);

    // Create real semaphores.
    // And place synchronization into `Schedule`.
    unimplemented!()
}

fn latest<R, S>(link: &Link<R>, schedule: &Schedule<S>) -> SubmitId
where
    R: Resource,
{
    let (_, sid) = link.queues()
        .map(|(qid, queue)| {
            let sid = SubmitId::new(qid, queue.last());
            (schedule[sid].wait_factor, sid)
        })
        .max_by_key(|&(wait_factor, sid)| (wait_factor, sid.queue().index()))
        .unwrap();
    sid
}

fn earliest<R, S>(link: &Link<R>, schedule: &Schedule<S>) -> SubmitId
where
    R: Resource,
{
    let (_, sid) = link.queues()
        .map(|(qid, queue)| {
            let sid = SubmitId::new(qid, queue.first());
            (schedule[sid].wait_factor, sid)
        })
        .min_by_key(|&(wait_factor, sid)| (wait_factor, sid.queue().index()))
        .unwrap();
    sid
}

fn sync_submit_chain<R, S>(
    sid: SubmitId,
    _submit: &Submit<S>,
    link_index: usize,
    id: Id<R>,
    chain: &Chain<R>,
    schedule: &Schedule<S>,
    sync: &mut Sync<Semaphore>,
) where
    R: Resource,
    Guard<Semaphore>: Pick<R, Target = Barriers<R>>,
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
            let this_earliest = earliest(this, schedule);
            if this_earliest == sid {
                // Find latest submit from prev chain.
                // It will release ownership.
                let prev_latest = latest(prev, schedule);

                // Wait for release.
                sync.acquire.wait.push(Wait::new(
                    Semaphore::new(
                        id,
                        Point::new(prev_latest, Side::Release)..Point::new(sid, Side::Acquire),
                    ),
                    this_state.stages,
                ));

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
                    sync.acquire.signal.push(Signal::new(Semaphore::new(
                        id,
                        Point::new(sid, Side::Acquire)
                            ..Point::new(SubmitId::new(qid, queue.first()), Side::Acquire),
                    )));
                }
            } else {
                // This is not the earliest.
                // Wait for earliest.
                sync.acquire.wait.push(Wait::new(
                    Semaphore::new(
                        id,
                        Point::new(this_earliest, Side::Acquire)..Point::new(sid, Side::Acquire),
                    ),
                    this_state.stages,
                ));
            }
        } else {
            // Same family.
            for tail in prev.tails() {
                if tail.queue() != sid.queue() {
                    // Wait for tails on other queues.
                    sync.acquire.wait.push(Wait::new(
                        Semaphore::new(
                            id,
                            Point::new(tail, Side::Release)..Point::new(sid, Side::Acquire),
                        ),
                        this_state.stages,
                    ));
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
            let this_latest = latest(this, schedule);
            if this_latest == sid {
                // Wait for other queues in this link.
                for (qid, queue) in this.queues().filter(|&(qid, _)| qid != sid.queue()) {
                    sync.release.wait.push(Wait::new(
                        Semaphore::new(
                            id,
                            Point::new(SubmitId::new(qid, queue.last()), Side::Release)
                                ..Point::new(sid, Side::Release),
                        ),
                        PipelineStage::TOP_OF_PIPE,
                    ));
                }

                // Find earliest submit from next chain.
                // It will acquire ownership.
                let next_earliest = earliest(next, schedule);

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
                sync.release.signal.push(Signal::new(Semaphore::new(
                    id,
                    Point::new(sid, Side::Release)..Point::new(next_earliest, Side::Acquire),
                )));
            } else {
                // This is not the latest.
                // Signal to latest.
                sync.release.signal.push(Signal::new(Semaphore::new(
                    id,
                    Point::new(sid, Side::Release)..Point::new(this_latest, Side::Release),
                )));
            }
        } else {
            // Same family.
            for head in next.heads() {
                if head.queue() != sid.queue() {
                    // Signal to heads on other queues.
                    sync.release.signal.push(Signal::new(Semaphore::new(
                        id,
                        Point::new(sid, Side::Release)..Point::new(head, Side::Acquire),
                    )));
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

fn sync_submit<S>(
    sid: SubmitId,
    submit: &Submit<S>,
    buffers: &BufferChains,
    images: &ImageChains,
    schedule: &Schedule<S>,
) -> Sync<Semaphore> {
    let mut sync = Sync::new();
    for (&id, &index) in submit.buffers.iter() {
        let ref chain = buffers[&id];
        sync_submit_chain(sid, submit, index, id, chain, schedule, &mut sync);
    }

    for (&id, &index) in submit.images.iter() {
        let ref chain = images[&id];
        sync_submit_chain(sid, submit, index, id, chain, schedule, &mut sync);
    }

    sync
}

fn optimize_submit(sid: SubmitId, sync: &mut HashMap<SubmitId, Sync<Semaphore>>) {
    use std::mem::replace;

    // Take all waits
    let mut acquire_wait = replace(&mut sync.get_mut(&sid).unwrap().acquire.wait, Vec::new());
    acquire_wait.sort_by_key(|wait| {
        assert_eq!(sid, wait.0.points().end.sid);
        (
            wait.stage(),
            usize::max_value() - wait.0.points().start.sid.index(),
        )
    });

    let mut release_wait = replace(&mut sync.get_mut(&sid).unwrap().release.wait, Vec::new());
    release_wait.sort_by_key(|wait| {
        assert_eq!(sid, wait.0.points().end.sid);
        (
            wait.stage(),
            usize::max_value() - wait.0.points().start.sid.index(),
        )
    });

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
                    .filter_map(|index| sync.get(&SubmitId::new(sid.queue(), index)))
                    .flat_map(|sync| sync.release.wait.iter().chain(sync.acquire.wait.iter())),
            )
            .any(|earlier| {
                // If waits for non-earlier point.
                earlier.0.points().start >= wait.0.points().start
            });

        if redundant {
            // Delete signal as well.
            let ref mut signal = sync.get_mut(&wait.0.points().start.sid)
                .unwrap()
                .get_mut(wait.0.points().start.side)
                .signal;
            let index = signal.iter().position(|signal| signal.0 == wait.0).unwrap();
            signal.remove(index);
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

fn optimize<S>(schedule: &Schedule<S>, sync: &mut HashMap<SubmitId, Sync<Semaphore>>) {
    for queue in schedule.iter().flat_map(|family| family.iter()) {
        let mut submits = queue.iter();
        while let Some((sid, _)) = submits.next_back() {
            optimize_submit(sid, sync);
        }
    }
}
