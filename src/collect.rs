//!
//! This module provides scheduling feature.
//! User can manually fill `Chains` structure.
//! `Chains` can be filled automatically by `schedule` function.
//!

use std::cmp::max;
use std::collections::HashMap;
use hal::queue::QueueFamilyId;

use chain::{BufferChains, Chain, ImageChains, Link};
use pass::{Pass, PassId};
use resource::{Resource, State};

use Pick;
use resource::Id;
use schedule::{QueueId, Schedule, Submit, SubmitId};

/// Placeholder for synchronization type.
pub struct UnSynchronized;

/// Result of pass scheduler.
pub struct Chains<S = UnSynchronized> {
    /// Contains submits for passes spread among queue schedule.
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

/// Calculate automatic `Chains` for passes.
/// This function tries to find most appropriate schedule for passes execution.
pub fn collect<F, Q>(passes: Vec<Pass>, max_queues: Q) -> Chains
where
    Q: Fn(QueueFamilyId) -> usize,
{
    // Indexed passes.
    let mut passes = passes
        .into_iter()
        .enumerate()
        .map(|(index, pass)| {
            pass.dependencies().to_owned().sort();
            (PassId(index), pass)
        })
        .collect::<Vec<_>>();

    // Track enqueued.
    let mut enqueued: Vec<usize> = Vec::new();

    // Chains.
    let mut images: ImageChains = ImageChains::new();
    let mut buffers: BufferChains = BufferChains::new();

    // Schedule
    let mut schedule = Schedule::default();

    while !passes.is_empty() {
        // Find passes that are ready to be enqueued
        // E.g. All dependencies are enqueued.
        enqueued.sort();
        // Among ready passes find best fit.
        let (fitness, qid, index) = passes
            .iter()
            .enumerate()
            .filter(|&(_, &(_, ref pass))| all_there(pass.dependencies(), &enqueued))
            .map(|(index, &(_, ref pass))| {
                let (fitness, qid) = fitness(
                    pass,
                    max_queues(pass.family()),
                    &mut images,
                    &mut buffers,
                    &schedule,
                );
                (fitness, qid, index)
            })
            .min()
            .unwrap();

        let (index, pass) = passes.swap_remove(index);

        schedule_pass(
            index,
            pass,
            qid,
            fitness.wait_factor,
            &mut schedule,
            &mut images,
            &mut buffers,
        );
    }

    Chains {
        schedule,
        buffers,
        images,
    }
}

fn all_there(all: &[usize], there: &[usize]) -> bool {
    let mut there = there.into_iter();
    for &a in all {
        if there.find(|&&t| t == a).is_none() {
            return false;
        }
    }
    true
}

fn fitness<S>(
    pass: &Pass,
    max_queues: usize,
    images: &mut ImageChains,
    buffers: &mut BufferChains,
    schedule: &Schedule<S>,
) -> (Fitness, QueueId) {
    // Find best queue for pass.
    pass.queue()
        .map_or((0..max_queues), |queue| queue..queue + 1)
        .map(|index| {
            let qid = QueueId::new(pass.family(), index);
            let sid = SubmitId::new(qid, schedule.get_queue(qid).map_or(0, |queue| queue.len()));

            let mut result = Fitness {
                transfers: 0,
                wait_factor: 0,
            };

            // Collect minimal waits required and resource transfers count.
            pass.buffers().for_each(|(id, &state)| {
                let (t, w) = buffers.get(id).map_or((0, 0), |chain| {
                    transfers_and_wait_factor(chain, sid, state, schedule)
                });
                result.transfers += t;
                result.wait_factor = max(result.wait_factor, w);
            });

            // Collect minimal waits required and resource transfers count.
            pass.images().for_each(|(id, &state)| {
                let (t, w) = images.get(id).map_or((0, 0), |chain| {
                    transfers_and_wait_factor(chain, sid, state, schedule)
                });
                result.transfers += t;
                result.wait_factor = max(result.wait_factor, w);
            });

            (result, qid)
        })
        .min()
        .unwrap()
}

fn transfers_and_wait_factor<R, S>(
    chain: &Chain<R>,
    sid: SubmitId,
    state: State<R>,
    schedule: &Schedule<S>,
) -> (usize, usize)
where
    R: Resource,
{
    let mut transfers = 0;
    let mut wait_factor = 0;

    let fake_link = Link::new(sid, state);
    if let Some(link) = chain.links().last() {
        transfers += if link.transfer(&fake_link) { 1 } else { 0 };

        for tail in link.tails() {
            wait_factor = max(schedule[tail].wait_factor, wait_factor);
        }
    }

    (transfers, wait_factor)
}

fn schedule_pass(
    pid: PassId,
    pass: Pass,
    qid: QueueId,
    wait_factor: usize,
    schedule: &mut Schedule<UnSynchronized>,
    images: &mut ImageChains,
    buffers: &mut BufferChains,
) {
    assert_eq!(qid.family(), pass.family());

    let ref mut queue = schedule.ensure_queue(qid);
    let sid = queue.add_submit(Submit::new(wait_factor, pid, UnSynchronized));
    let ref mut submit = queue[sid];

    for (&id, &state) in pass.buffers() {
        let chain = buffers.entry(id).or_insert_with(|| Chain::default());
        add_to_chain(id, chain, sid, submit, state);
    }

    for (&id, &state) in pass.images() {
        let chain = images.entry(id).or_insert_with(|| Chain::default());
        add_to_chain(id, chain, sid, submit, state);
    }
}

fn add_to_chain<R, S>(
    id: Id<R>,
    chain: &mut Chain<R>,
    sid: SubmitId,
    submit: &mut Submit<S>,
    state: State<R>,
) where
    R: Resource,
    Submit<S>: Pick<R, Target = HashMap<Id<R>, usize>>,
{
    let chain_len = chain.links().len();
    let append = match chain.last_link_mut() {
        Some(ref mut link) if link.compatible(sid, state) => {
            submit.pick_mut().insert(id, chain_len - 1);
            link.insert_submit(sid, state);
            None
        }
        Some(_) | None => {
            submit.pick_mut().insert(id, chain_len);
            Some(Link::new(sid, state))
        }
    };

    if let Some(link) = append {
        chain.add_link(link);
    }
}
