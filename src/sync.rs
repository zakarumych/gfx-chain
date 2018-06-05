//!
//! This crates provide functions for find all required synchronizations (barriers and semaphores).
//!

use fnv::FnvHashMap;
use std::cmp::Ordering;
use std::ops::{Range, RangeFrom, RangeTo};

use hal::pso::PipelineStage;

use Pick;
use chain::{Chain, Link};
use collect::{Chains, Unsynchronized};
use resource::{Access, Buffer, Id, Image, Resource, State};
use schedule::{QueueId, Schedule, SubmissionId};

// fn earlier_stage(stages: PipelineStage) -> PipelineStage {
//     PipelineStage::from_bits((stages.bits() - 1) ^ (stages.bits())).unwrap()
// }

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
    fn new(id: Uid, points: Range<Point>) -> Self {
        Semaphore {
            id,
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
        Wait(semaphore, stages)
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
pub type Barriers<R> = FnvHashMap<Id<R>, Barrier<R>>;

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
            buffers: FnvHashMap::default(),
            images: FnvHashMap::default(),
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

struct SyncTemp(FnvHashMap<SubmissionId, SyncData<Semaphore, Semaphore>>);
impl SyncTemp {
    fn get_sync(&mut self, sid: SubmissionId) -> &mut SyncData<Semaphore, Semaphore> {
        self.0.entry(sid).or_insert_with(|| SyncData::new())
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

    let mut sync = SyncTemp(FnvHashMap::default());
    for (&id, chain) in buffers {
        sync_chain(id, chain, schedule, &mut sync);
    }
    for (&id, chain) in images {
        sync_chain(id, chain, schedule, &mut sync);
    }

    if schedule.queue_count() > 1 {
        optimize(schedule, &mut sync);
    }

    let mut result = Schedule::default();
    let mut signals: FnvHashMap<Semaphore, Option<S>> = FnvHashMap::default();
    let mut waits: FnvHashMap<Semaphore, Option<W>> = FnvHashMap::default();

    for queue in schedule.iter().flat_map(|family| family.iter()) {
        let new_queue = result.ensure_queue(queue.id());
        for (sid, submission) in queue.iter() {
            let sync = if let Some(sync) = sync.0.remove(&sid) {
                let sync = sync.convert_signal(|semaphore| {
                    match signals.get_mut(&semaphore) {
                        None => {
                            let (signal, wait) = new_semaphore();
                            let old = waits.insert(semaphore, Some(wait));
                            assert!(old.is_none());
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
                            let old = signals.insert(semaphore, Some(signal));
                            assert!(old.is_none());
                            wait
                        }
                        Some(wait) => {
                            wait.take().unwrap()
                        }
                    }
                });
                sync
            } else {
                SyncData::new()
            };
            let new_sid = new_queue.add_submission(submission.set_sync(sync));
            assert_eq!(sid, new_sid);
        }
    }

    debug_assert!(sync.0.is_empty());
    debug_assert!(signals.values().all(|x| x.is_none()));
    debug_assert!(waits.values().all(|x| x.is_none()));

    result
}

fn latest<R, S>(link: &Link<R>, schedule: &Schedule<S>) -> SubmissionId
where
    R: Resource,
{
    let (_, sid) = link.queues()
        .map(|(qid, queue)| {
            let sid = SubmissionId::new(qid, queue.last);
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
            let sid = SubmissionId::new(qid, queue.first);
            (schedule[sid].wait_factor(), sid)
        })
        .min_by_key(|&(wait_factor, sid)| (wait_factor, sid.queue().index()))
        .unwrap();
    sid
}

fn generate_semaphore_pair<R: Resource>(
    sync: &mut SyncTemp, id: Uid, link: &Link<R>,
    range: Range<SubmissionId>, sides: Range<Side>,
) {
    let points = Point::new(range.start, sides.start) .. Point::new(range.end, sides.end);
    if points.start.sid.queue() != points.end.sid.queue() {
        let semaphore = Semaphore::new(id, points.clone());
        sync.get_sync(points.start.sid).get_mut(points.start.side).signal
            .push(Signal::new(semaphore.clone()));
        sync.get_sync(points.end.sid).get_mut(points.end.side).wait
            .push(Wait::new(semaphore, link.queue(points.end.sid.queue()).stages));
    }
}

fn sync_chain<R, S>(
    id: Id<R>,
    chain: &Chain<R>,
    schedule: &Schedule<S>,
    sync: &mut SyncTemp,
) where
    R: Resource,
    Id<R>: Into<Uid>,
    Guard<Semaphore, Semaphore>: Pick<R, Target = Barriers<R>>,
{
    let uid = id.into();
    for (prev_link, link) in chain.links().windows(2).map(|pair| (&pair[0], &pair[1])) {
        if prev_link.family() == link.family() {
            // Prefer to generate barriers on the acquire side, if possible.
            if prev_link.single_queue() && !link.single_queue() {
                let signal_sid = latest(prev_link, schedule);

                // Generate barrier in prev link's last submission.
                sync.get_sync(signal_sid).release.pick_mut()
                    .insert(id, Barrier::new(prev_link.state()..link.state()));

                // Generate semaphores between queues in the previous link and the current one.
                for (queue_id, queue) in link.queues() {
                    let head = SubmissionId::new(queue_id, queue.first);
                    generate_semaphore_pair(
                        sync, uid, link,
                        signal_sid .. head, Side::Release .. Side::Acquire,
                    );
                }
            } else {
                let wait_sid = earliest(link, schedule);

                // Generate semaphores between queues in the previous link and the current one.
                for (queue_id, queue) in prev_link.queues() {
                    let tail = SubmissionId::new(queue_id, queue.last);
                    generate_semaphore_pair(
                        sync, uid, link,
                        tail .. wait_sid, Side::Release .. Side::Acquire,
                    );
                }

                // Generate barrier in next link's first submission.
                sync.get_sync(wait_sid).acquire.pick_mut()
                    .insert(id, Barrier::new(prev_link.state()..link.state()));

                if !link.single_queue() {
                    // Delay other queues in the link until the barrier finishes
                    for (queue_id, queue) in link.queues() {
                        if queue_id != wait_sid.queue() {
                            let head = SubmissionId::new(queue_id, queue.first);
                            generate_semaphore_pair(
                                sync, uid, link,
                                wait_sid .. head, Side::Acquire .. Side::Acquire,
                            );
                        }
                    }
                }
            }
        } else {
            let signal_sid = latest(prev_link, schedule);
            let wait_sid = earliest(link, schedule);

            if !prev_link.single_queue() {
                // Delay the last submission in the queue until other queues finish
                for (queue_id, queue) in link.queues() {
                    if queue_id != signal_sid.queue() {
                        let tail = SubmissionId::new(queue_id, queue.last);
                        generate_semaphore_pair(
                            sync, uid, prev_link,
                            tail .. signal_sid, Side::Release .. Side::Release,
                        );
                    }
                }
            }

            // Generate a semaphore between the signal and wait sides of the transfer.
            generate_semaphore_pair(
                sync, uid, link,
                signal_sid .. wait_sid, Side::Release .. Side::Acquire,
            );

            // Generate barriers to transfer the resource to another queue.
            sync.get_sync(signal_sid).release.pick_mut()
                .insert(id, Barrier::release(
                    signal_sid.queue() .. wait_sid.queue(),
                    prev_link.queue_state(signal_sid.queue()) ..,
                    .. link.state().layout,
                ));
            sync.get_sync(wait_sid).acquire.pick_mut()
                .insert(id, Barrier::acquire(
                    signal_sid.queue() .. wait_sid.queue(),
                    prev_link.state().layout ..,
                    .. link.queue_state(wait_sid.queue()),
                ));

            if !link.single_queue() {
                // Delay other queues in the link until the barrier finishes
                for (queue_id, queue) in link.queues() {
                    if queue_id != wait_sid.queue() {
                        let head = SubmissionId::new(queue_id, queue.first);
                        generate_semaphore_pair(
                            sync, uid, link,
                            wait_sid .. head, Side::Acquire .. Side::Acquire,
                        );
                    }
                }
            }
        }
    }
}

fn optimize_side(
    guard: &mut Guard<Semaphore, Semaphore>,
    to_remove: &mut Vec<Semaphore>,
    found: &mut FnvHashMap<QueueId, (usize, Side)>,
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
    to_remove: &mut Vec<Semaphore>,
    found: &mut FnvHashMap<QueueId, (usize, Side)>,
    sync: &mut SyncTemp,
) {
    {
        let sync_data = sync.0.get_mut(&sid);
        if let Some(sync_data) = sync_data {
            optimize_side(&mut sync_data.acquire, to_remove, found);
            optimize_side(&mut sync_data.release, to_remove, found);
        } else {
            return
        }
    }

    for semaphore in to_remove.drain(..) {
        // Delete signal as well.
        let ref mut signal = sync.0.get_mut(&semaphore.points.start.sid)
            .unwrap()
            .get_mut(semaphore.points.start.side)
            .signal;
        let index = signal.iter().position(|signal| signal.0 == semaphore).unwrap();
        signal.swap_remove(index);
    }
}

fn optimize<S>(
    schedule: &Schedule<S>,
    sync: &mut SyncTemp,
) {
    let mut to_remove = Vec::new();
    for queue in schedule.iter().flat_map(|family| family.iter()) {
        let mut found = FnvHashMap::default();
        for (sid, _) in queue.iter() {
            optimize_submission(sid, &mut to_remove, &mut found, sync);
        }
    }
}
