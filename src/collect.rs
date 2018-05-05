//!
//! This module provides scheduling feature.
//! User can manually fill `Chains` structure.
//! `Chains` can be filled automatically by `schedule` function.
//!

use std::cmp::max;
use std::collections::HashMap;
use std::hash::Hash;
use std::ops::Range;
use hal::queue::QueueFamilyId;

use chain::{BufferChains, Chain, ImageChains, Link};
use pass::{Pass, PassId, StateUsage};
use resource::{Buffer, Image, Resource, State};

use Pick;
use resource::Id;
use schedule::{Queue, QueueId, Schedule, Submission, SubmissionId};

/// Placeholder for synchronization type.
#[derive(Debug)]
pub struct Unsynchronized;

/// Result of pass scheduler.
#[derive(Debug)]
pub struct Chains<S = Unsynchronized> {
    /// Contains submissions for passes spread among queue schedule.
    pub schedule: Schedule<S>,

    /// Contains all buffer chains.
    pub buffers: BufferChains,

    /// Contains all image chains.
    pub images: ImageChains,
}

#[derive(PartialEq, PartialOrd, Eq, Ord)]
struct Fitness {
    transfers: usize,
    wait_factor: usize,
}

struct ResolvedPass {
    id: usize,
    family: QueueFamilyId,
    queues: Range<usize>,
    deps: Vec<usize>,
    buffers: Vec<(usize, StateUsage<Buffer>)>,
    images: Vec<(usize, StateUsage<Image>)>,
}

struct ResolvedPassSet {
    passes: Vec<ResolvedPass>,
    pass_ids: Vec<PassId>,
    queues: Vec<QueueId>,
    buffers: Vec<Id<Buffer>>,
    images: Vec<Id<Image>>,
}

struct ChainData<R: Resource> {
    chain: Chain<R>,
    last_link_wait_factor: usize,
    current_link_wait_factor: usize,
    current_family: Option<QueueFamilyId>,
}
impl <R: Resource> Default for ChainData<R> {
    fn default() -> Self {
        ChainData {
            chain: Chain::new(),
            last_link_wait_factor: 0,
            current_link_wait_factor: 0,
            current_family: None,
        }
    }
}

struct QueueData {
    queue: Queue<Unsynchronized>,
    wait_factor: usize,
}

/// Calculate automatic `Chains` for passes.
/// This function tries to find most appropriate schedule for passes execution.
pub fn collect<Q>(passes: Vec<Pass>, max_queues: Q) -> Chains
where
    Q: Fn(QueueFamilyId) -> usize,
{
    // Resolve passes into a form faster to work with.
    let mut passes = resolve_passes(passes, max_queues);

    // Track scheduled.
    let mut scheduled: Vec<bool> = fill(passes.pass_ids.len());

    // Chains.
    let mut images: Vec<ChainData<Image>> = fill(passes.images.len());
    let mut buffers: Vec<ChainData<Buffer>> = fill(passes.buffers.len());

    // Schedule
    let mut schedule = Vec::with_capacity(passes.queues.len());
    for i in 0..passes.queues.len() {
        schedule.push(QueueData {
            queue: Queue::new(passes.queues[i]),
            wait_factor: 0,
        });
    }

    while !passes.passes.is_empty() {
        // Among ready passes find best fit.
        let (fitness, qid, index) = passes.passes
            .iter()
            .enumerate()
            .filter(|&(_, pass)| pass.deps.iter().all(|&x| scheduled[x]))
            .map(|(index, pass)| {
                let (fitness, qid) = fitness(
                    pass,
                    &mut images,
                    &mut buffers,
                    &mut schedule,
                );
                (fitness, qid, index)
            })
            .min()
            .unwrap();

        let pass = passes.passes.swap_remove(index);
        scheduled[pass.id] = true;

        schedule_pass(
            &passes,
            pass,
            qid,
            fitness.wait_factor,
            &mut schedule,
            &mut images,
            &mut buffers,
        );
    }

    Chains {
        schedule: reify_schedule(&passes.queues, schedule),
        buffers: reify_chain(&passes.buffers, buffers),
        images: reify_chain(&passes.images, images),
    }
}

fn fill<T: Default>(num: usize) -> Vec<T> {
    let mut vec = Vec::with_capacity(num);
    for _ in 0..num {
        vec.push(T::default());
    }
    vec
}

struct LookupBuilder<I : Hash + Eq + Copy> {
    forward: HashMap<I, usize>,
    backward: Vec<I>,
}
impl <I : Hash + Eq + Copy> LookupBuilder<I> {
    fn new() -> LookupBuilder<I> {
        LookupBuilder { forward: HashMap::new(), backward: Vec::new() }
    }
    fn get(&mut self, id: I) -> Option<usize> {
        self.forward.get(&id).cloned()
    }
    fn forward(&mut self, id: I) -> usize {
        if let Some(&id_num) = self.forward.get(&id) {
            id_num
        } else {
            let id_num = self.backward.len();
            self.backward.push(id);
            self.forward.insert(id, id_num);
            id_num
        }
    }
}

fn resolve_passes<Q>(passes: Vec<Pass>, max_queues: Q) -> ResolvedPassSet
where
    Q: Fn(QueueFamilyId) -> usize,
{
    let mut reified_passes = Vec::new();
    let mut pass_ids = LookupBuilder::new();
    let mut queues = LookupBuilder::new();
    let mut buffers = LookupBuilder::new();
    let mut images = LookupBuilder::new();

    let mut family_full = HashMap::new();
    for pass in passes {
        let family = pass.family;
        if !family_full.contains_key(&family) {
            let count = max_queues(family);
            assert!(count > 0, "Cannot create a family with 0 max queues.");
            for i in 0..count {
                queues.forward(QueueId::new(family, i));
            }

            let full_range = queues.forward(QueueId::new(family, 0)) ..
                             queues.forward(QueueId::new(family, count - 1)) + 1;
            family_full.insert(family, full_range);
        }

        reified_passes.push(ResolvedPass {
            id: pass_ids.forward(pass.id),
            family: pass.family,
            queues: if let Some(queue) = pass.queue {
                let id = queues
                    .get(QueueId::new(family, queue))
                    .expect("Requested queue out of range!");
                id .. id + 1
            } else {
                family_full[&family].clone()
            },
            deps: pass.dependencies.into_iter().map(|p| pass_ids.forward(p)).collect(),
            buffers: pass.buffers.into_iter().map(|(k, v)| (buffers.forward(k), v)).collect(),
            images: pass.images.into_iter().map(|(k, v)| (images.forward(k), v)).collect(),
        });
    }

    ResolvedPassSet {
        passes: reified_passes,
        pass_ids: pass_ids.backward,
        queues: queues.backward,
        buffers: buffers.backward,
        images: images.backward,
    }
}

fn reify_chain<R: Resource>(ids: &[Id<R>], vec: Vec<ChainData<R>>) -> HashMap<Id<R>, Chain<R>> {
    let mut map = HashMap::with_capacity(vec.len());
    for (chain, &i) in vec.into_iter().zip(ids) {
        map.insert(i, chain.chain);
    }
    map
}

fn reify_schedule(ids: &[QueueId], vec: Vec<QueueData>) -> Schedule<Unsynchronized> {
    let mut schedule = Schedule::new();
    for (queue_data, &i) in vec.into_iter().zip(ids) {
        *schedule.ensure_queue(i) = queue_data.queue;
    }
    schedule
}

fn fitness(
    pass: &ResolvedPass,
    images: &mut Vec<ChainData<Image>>,
    buffers: &mut Vec<ChainData<Buffer>>,
    schedule: &mut Vec<QueueData>,
) -> (Fitness, usize) {
    let mut transfers = 0;
    let mut wait_factor_from_chains = 0;

    // Collect minimal waits required and resource transfers count.
    for &(id, _) in &pass.buffers {
        let chain = &buffers[id];
        if chain.current_family.unwrap_or(pass.family) != pass.family {
            transfers += 1;
        }
        wait_factor_from_chains = max(wait_factor_from_chains, chain.last_link_wait_factor);
    }
    for &(id, _) in &pass.images {
        let chain = &images[id];
        if chain.current_family.unwrap_or(pass.family) != pass.family {
            transfers += 1;
        }
        wait_factor_from_chains = max(wait_factor_from_chains, chain.last_link_wait_factor);
    }

    // Find best queue for pass.
    let (wait_factor_from_queue, queue) =
        pass.queues.clone()
            .map(|index| (schedule[index].wait_factor, index))
            .min()
            .unwrap();
    (Fitness {
        transfers,
        wait_factor: max(wait_factor_from_chains, wait_factor_from_queue),
    }, queue)
}

fn schedule_pass(
    passes: &ResolvedPassSet,
    pass: ResolvedPass,
    queue: usize,
    wait_factor: usize,
    schedule: &mut Vec<QueueData>,
    images: &mut Vec<ChainData<Image>>,
    buffers: &mut Vec<ChainData<Buffer>>,
) {
    let pid = passes.pass_ids[pass.id];
    let ref mut queue_data = schedule[queue];
    queue_data.wait_factor = max(queue_data.wait_factor, wait_factor + 1);
    let sid = queue_data.queue.add_submission(Submission::new(wait_factor, pid, Unsynchronized));
    let ref mut submission = queue_data.queue[sid];

    for (id, StateUsage { state, usage }) in pass.buffers {
        add_to_chain(
            passes.buffers[id], pass.family, &mut buffers[id], sid, submission, state, usage,
        );
    }
    for (id, StateUsage { state, usage }) in pass.images {
        add_to_chain(
            passes.images[id], pass.family, &mut images[id], sid, submission, state, usage,
        );
    }
}

fn add_to_chain<R, S>(
    id: Id<R>,
    family: QueueFamilyId,
    chain_data: &mut ChainData<R>,
    sid: SubmissionId,
    submission: &mut Submission<S>,
    state: State<R>,
    usage: R::Usage,
) where
    R: Resource,
    Submission<S>: Pick<R, Target = HashMap<Id<R>, usize>>,
{
    chain_data.current_family = Some(family);
    chain_data.current_link_wait_factor =
        max(submission.wait_factor() + 1, chain_data.current_link_wait_factor);

    let ref mut chain = chain_data.chain;
    let chain_len = chain.links().len();
    let append = match chain.last_link_mut() {
        Some(ref mut link) if link.compatible(sid, state) => {
            submission.pick_mut().insert(id, chain_len - 1);
            link.insert_submission(sid, state, usage);
            None
        }
        Some(_) | None => {
            submission.pick_mut().insert(id, chain_len);
            chain_data.last_link_wait_factor = chain_data.current_link_wait_factor;
            Some(Link::new(sid, state, usage))
        }
    };

    if let Some(link) = append {
        chain.add_link(link);
    }
}
