//!
//! This crates provide functions for find all required synchronizations (barriers and semaphores).
//!

use std::cmp::Ordering;
use std::collections::HashMap;
use std::ops::{Range, RangeFrom, RangeTo};

use hal::pso::PipelineStage;

use Pick;
use chain::{BufferChains, Chain, ImageChains, Link};
use collect::{Chains, Unsynchronized};
use resource::{Access, Buffer, Id, Image, Resource, State};
use schedule::{QueueId, Schedule, Submission, SubmissionId};

fn earlier_stage(stages: PipelineStage) -> PipelineStage {
    PipelineStage::from_bits((stages.bits() - 1) ^ (stages.bits())).unwrap()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
enum Uid {
    Buffer(u32),
    Image(u32),
}

impl From<Id<Buffer>> for Uid {
    fn from(id: Id<Buffer>) -> Uid {
        Uid::Buffer(id.index())
    }
}

impl From<Id<Image>> for Uid {
    fn from(id: Id<Image>) -> Uid {
        Uid::Image(id.index())
    }
}


/// Side of the submission. `Acquire` or `Release`.
#[derive(Clone, Copy, Debug, PartialOrd, Ord, PartialEq, Eq, Hash)]
enum Side {
    /// Acquire side of the submission.
    /// Synchronization commands from this side must be recorded before main commands of submission.
    Acquire,

    /// Release side of the submission.
    /// Synchronization commands from this side must be recorded after main commands of submission.
    Release,
}

/// Point in execution graph. Target submission and side.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct Point {
    /// Id of submission.
    sid: SubmissionId,

    /// Side of the submission.
    side: Side,
}

impl Point {
    fn new(sid: SubmissionId, side: Side) -> Self {
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
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct Semaphore {
    id: Uid,
    points: Range<Point>,
}

impl Semaphore {
    fn new<R>(id: Id<R>, points: Range<Point>) -> Self
    where
        R: Resource,
        Id<R>: Into<Uid>,
    {
        Semaphore {
            id: id.into(),
            points,
        }
    }
}

/// Semaphore signal info.
/// There must be paired wait.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
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
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
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
#[derive(Clone, Debug)]
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
pub type Barriers<R> = HashMap<Id<R>, Barrier<R>>;

/// Map of barriers by buffer id.
pub type BufferBarriers = Barriers<Buffer>;

/// Map of barriers by image id.
pub type ImageBarriers = Barriers<Image>;

/// Synchronization for submission at one side.
#[derive(Clone, Debug)]
pub struct Guard<S, W> {
    /// Points at other queues that must be waited before commands from the submission can be executed.
    pub wait: Vec<Wait<W>>,

    /// Buffer pipeline barriers to be inserted before or after (depends on the side) main commands of the submission.
    pub buffers: BufferBarriers,

    /// Image pipeline barriers to be inserted before or after (depends on the side) main commands of the submission.
    pub images: ImageBarriers,

    /// Points at other queues that can run after barriers above.
    pub signal: Vec<Signal<S>>,
}

impl<S, W> Guard<S, W> {
    fn new() -> Self {
        Guard {
            wait: Vec::new(),
            buffers: HashMap::new(),
            images: HashMap::new(),
            signal: Vec::new(),
        }
    }
}

impl<S, W> Pick<Image> for Guard<S, W> {
    type Target = ImageBarriers;

    fn pick(&self) -> &ImageBarriers {
        &self.images
    }
    fn pick_mut(&mut self) -> &mut ImageBarriers {
        &mut self.images
    }
}

impl<S, W> Pick<Buffer> for Guard<S, W> {
    type Target = BufferBarriers;

    fn pick(&self) -> &BufferBarriers {
        &self.buffers
    }
    fn pick_mut(&mut self) -> &mut BufferBarriers {
        &mut self.buffers
    }
}

/// Both sides of synchronization for submission.
#[derive(Clone, Debug)]
pub struct SyncData<S, W> {
    /// Acquire side of submission synchronization.
    /// Synchronization commands from this side must be recorded before main commands of submission.
    pub acquire: Guard<S, W>,
    /// Release side of submission synchronization.
    /// Synchronization commands from this side must be recorded after main commands of submission.
    pub release: Guard<S, W>,
}

impl<S, W> SyncData<S, W> {
    fn new() -> Self {
        SyncData {
            acquire: Guard::new(),
            release: Guard::new(),
        }
    }

    /// Get mutable reference to `Guard` by `Side`.
    fn get_mut(&mut self, side: Side) -> &mut Guard<S, W> {
        match side {
            Side::Acquire => &mut self.acquire,
            Side::Release => &mut self.release,
        }
    }

    fn convert_signal<F, T>(self, mut f: F) -> SyncData<T, W>
    where
        F: FnMut(S) -> T,
    {
        SyncData {
            acquire: Guard {
                wait: self.acquire.wait,
                signal: self.acquire.signal.into_iter().map(|Signal(semaphore)| Signal(f(semaphore))).collect(),
                buffers: self.acquire.buffers,
                images: self.acquire.images,
            },
            release: Guard {
                wait: self.release.wait,
                signal: self.release.signal.into_iter().map(|Signal(semaphore)| Signal(f(semaphore))).collect(),
                buffers: self.release.buffers,
                images: self.release.images,
            },
        }
    }

    fn convert_wait<F, T>(self, mut f: F) -> SyncData<S, T>
    where
        F: FnMut(W) -> T,
    {
        SyncData {
            acquire: Guard {
                wait: self.acquire.wait.into_iter().map(|Wait(semaphore, stage)| Wait(f(semaphore), stage)).collect(),
                signal: self.acquire.signal,
                buffers: self.acquire.buffers,
                images: self.acquire.images,
            },
            release: Guard {
                wait: self.release.wait.into_iter().map(|Wait(semaphore, stage)| Wait(f(semaphore), stage)).collect(),
                signal: self.release.signal,
                buffers: self.release.buffers,
                images: self.release.images,
            },
        }
    }
}

/// Find required synchronization for all submissions in `Chains`.
pub fn sync<F, S, W>(
    chains: &Chains<Unsynchronized>,
    mut new_semaphore: F,
) -> Schedule<SyncData<S, W>> where
    F: FnMut() -> (S, W),
{
    let ref schedule = chains.schedule;
    let ref buffers = chains.buffers;
    let ref images = chains.images;

    let mut sync = schedule
        .iter()
        .flat_map(|family| family.iter())
        .flat_map(|queue| queue.iter())
        .map(|(sid, submission)| (sid, sync_submission(sid, submission, buffers, images, schedule)))
        .collect();

    optimize(schedule, &mut sync);

    let mut result = Schedule::default();
    let mut signals: HashMap<Semaphore, Option<S>> = HashMap::new();
    let mut waits: HashMap<Semaphore, Option<W>> = HashMap::new();

    for queue in schedule.iter().flat_map(|family| family.iter()) {
        let new_queue = result.ensure_queue(queue.id());
        for (sid, submission) in queue.iter() {
            let sync = sync.remove(&sid).unwrap();
            let sync = sync.convert_signal(|semaphore| {
                match signals.get_mut(&semaphore) {
                    None => {
                        let (signal, wait) = new_semaphore();
                        assert!(waits.insert(semaphore, Some(wait)).is_none());
                        signal
                    }
                    Some(signal) => {
                        signal.take().unwrap()
                    }
                }
            });
            let sync = sync.convert_wait(|semaphore| {
                match waits.get_mut(&semaphore) {
                    None => {
                        let (signal, wait) = new_semaphore();
                        assert!(signals.insert(semaphore, Some(signal)).is_none());
                        wait
                    }
                    Some(wait) => {
                        wait.take().unwrap()
                    }
                }
            });
            assert_eq!(sid, new_queue.add_submission(submission.set_sync(sync)));
        }
    }

    assert!(signals.values().all(|x| x.is_none()));
    assert!(waits.values().all(|x| x.is_none()));

    result
}

fn latest<R, S>(link: &Link<R>, schedule: &Schedule<S>) -> SubmissionId
where
    R: Resource,
{
    let (_, sid) = link.queues()
        .map(|(qid, queue)| {
            let sid = SubmissionId::new(qid, queue.last());
            (schedule[sid].wait_factor(), sid)
        })
        .max_by_key(|&(wait_factor, sid)| (wait_factor, sid.queue().index()))
        .unwrap();
    sid
}

fn earliest<R, S>(link: &Link<R>, schedule: &Schedule<S>) -> SubmissionId
where
    R: Resource,
{
    let (_, sid) = link.queues()
        .map(|(qid, queue)| {
            let sid = SubmissionId::new(qid, queue.first());
            (schedule[sid].wait_factor(), sid)
        })
        .min_by_key(|&(wait_factor, sid)| (wait_factor, sid.queue().index()))
        .unwrap();
    sid
}

fn sync_submission_chain<R, S>(
    sid: SubmissionId,
    _submission: &Submission<S>,
    link_index: usize,
    id: Id<R>,
    chain: &Chain<R>,
    schedule: &Schedule<S>,
    sync: &mut SyncData<Semaphore, Semaphore>,
) where
    R: Resource,
    Id<R>: Into<Uid>,
    Guard<Semaphore, Semaphore>: Pick<R, Target = Barriers<R>>,
{
    let ref this = chain.links()[link_index];
    assert_eq!(this.family(), sid.family());
    let ref queue = this.queue(sid.queue()).unwrap();
    assert!(queue.first() <= sid.index());
    assert!(queue.last() >= sid.index());

    let this_state = this.queue_state(sid.queue());

    if queue.first() == sid.index() && link_index > 0 {
        // First of queue. Sync with prev.
        let ref prev = chain.links()[link_index - 1];
        if prev.family() != this.family() && this.state().access.is_read() {
            // Transfer ownership.
            // Find earliest submission from this chain.
            // It will acquire ownership.
            let this_earliest = earliest(this, schedule);
            if this_earliest == sid {
                // Find latest submission from prev chain.
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
                        prev.state().layout..,
                        ..this_state,
                    ),
                );

                // Signal to other queues in this link.
                for (qid, queue) in this.queues().filter(|&(qid, _)| qid != sid.queue()) {
                    sync.acquire.signal.push(Signal::new(Semaphore::new(
                        id,
                        Point::new(sid, Side::Acquire)
                            ..Point::new(SubmissionId::new(qid, queue.first()), Side::Acquire),
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
            // Same family or content discarding.
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
                } else if !prev.single_queue() {
                    // Insert barrier here.
                    // Prev won't insert as it isn't exclusive.
                    assert!(this.single_queue(),
                            "Barriers cannot currently be inserted between links that both \
                             involve more than one queue.");
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
        if next.family() != this.family() && next.state().access.is_read() {
            // Transfer ownership.

            // Find latest submission from this chain.
            // It will release ownership.
            let this_latest = latest(this, schedule);
            if this_latest == sid {
                // Wait for other queues in this link.
                for (qid, queue) in this.queues().filter(|&(qid, _)| qid != sid.queue()) {
                    sync.release.wait.push(Wait::new(
                        Semaphore::new(
                            id,
                            Point::new(SubmissionId::new(qid, queue.last()), Side::Release)
                                ..Point::new(sid, Side::Release),
                        ),
                        PipelineStage::TOP_OF_PIPE,
                    ));
                }

                // Find earliest submission from next chain.
                // It will acquire ownership.
                let next_earliest = earliest(next, schedule);

                // Release ownership.
                sync.release.pick_mut().insert(
                    id,
                    Barrier::release(
                        sid.queue()..next_earliest.queue(),
                        this_state..,
                        ..next.state().layout,
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
            // Same family or content discarding.
            for head in next.heads() {
                if head.queue() != sid.queue() {
                    // Signal to heads on other queues.
                    sync.release.signal.push(Signal::new(Semaphore::new(
                        id,
                        Point::new(sid, Side::Release)..Point::new(head, Side::Acquire),
                    )));
                } else if this.single_queue() {
                    // Insert barrier here.
                    // Next won't insert as this is exclusive.
                    sync.release
                        .pick_mut()
                        .insert(id, Barrier::new(this_state..next.queue_state(head.queue())));
                } else {
                    // Next will insert barrier.
                    assert!(next.single_queue(),
                            "Barriers cannot currently be inserted between links that both \
                             involve more than one queue.");
                }
            }
        }
    }
}

fn sync_submission<S>(
    sid: SubmissionId,
    submission: &Submission<S>,
    buffers: &BufferChains,
    images: &ImageChains,
    schedule: &Schedule<S>,
) -> SyncData<Semaphore, Semaphore> {
    let mut sync = SyncData::new();
    for (&id, &index) in submission.buffers() {
        let ref chain = buffers[&id];
        sync_submission_chain(sid, submission, index, id, chain, schedule, &mut sync);
    }

    for (&id, &index) in submission.images() {
        let ref chain = images[&id];
        sync_submission_chain(sid, submission, index, id, chain, schedule, &mut sync);
    }

    sync
}

fn optimize_side(
    guard: &mut Guard<Semaphore, Semaphore>,
    to_remove: &mut Vec<Semaphore>,
    found: &mut HashMap<QueueId, (usize, Side)>,
) {
    guard.wait.sort_unstable_by_key(
        |wait| (wait.stage(),
                wait.semaphore().points.end.sid.index())
    );
    guard.wait.retain(|wait| {
        let start = wait.semaphore().points.start;
        let pos = (start.sid.index(), start.side);
        if let Some(synched_to) = found.get_mut(&start.sid.queue()) {
            if *synched_to >= pos {
                to_remove.push(wait.semaphore().clone());
                return false
            } else {
                *synched_to = pos;
                return true
            }
        }

        found.insert(start.sid.queue(), pos);
        true
    });
}

fn optimize_submission(
    sid: SubmissionId,
    found: &mut HashMap<QueueId, (usize, Side)>,
    sync: &mut HashMap<SubmissionId, SyncData<Semaphore, Semaphore>>,
) {
    let mut to_remove = Vec::new();

    {
        let sync_data = sync.get_mut(&sid).unwrap();
        optimize_side(&mut sync_data.acquire, &mut to_remove, found);
        optimize_side(&mut sync_data.release, &mut to_remove, found);
    }

    for semaphore in to_remove {
        // Delete signal as well.
        let ref mut signal = sync.get_mut(&semaphore.points.start.sid)
            .unwrap()
            .get_mut(semaphore.points.start.side)
            .signal;
        let index = signal.iter().position(|signal| signal.0 == semaphore).unwrap();
        signal.swap_remove(index);
    }
}

fn optimize<S>(
    schedule: &Schedule<S>,
    sync: &mut HashMap<SubmissionId, SyncData<Semaphore, Semaphore>>,
) {
    for queue in schedule.iter().flat_map(|family| family.iter()) {
        let mut found = HashMap::new();
        for (sid, _) in queue.iter() {
            optimize_submission(sid, &mut found, sync);
        }
    }
}
