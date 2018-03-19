//!
//! This module provides scheduling feature.
//! User can manually fill `Schedule` structure.
//! `Schedule` can be filled automatically by `schedule` function.
//! 

use std::cmp::max;
use hal::queue::QueueFamilyId;

use chain::{BufferChains, Chain, ImageChains, Link};
use pass::{Pass as PassDesc, PassId};
use resource::{Resource, State};

use resource::Id;
use families::{Families, QueueId, Submit, SubmitId, SubmitInsertLink};


/// Result of pass scheduler.
pub struct Schedule {
    /// Contains submits for passes spread among queue families.
    pub families: Families,

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

/// Calculate automatic `Schedule` for passes.
/// This function tries to find most appropriate schedule for passes execution.
pub fn schedule<F, Q>(passes: Vec<PassDesc>, max_queues: Q) -> Schedule
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

    // Families
    let mut families = Families::default();

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
                    &families,
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
            &mut families,
            &mut images,
            &mut buffers,
        );
    }

    Schedule {
        families,
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

fn fitness(
    pass: &PassDesc,
    max_queues: usize,
    images: &mut ImageChains,
    buffers: &mut BufferChains,
    families: &Families,
) -> (Fitness, QueueId) {
    // Find best queue for pass.
    pass.queue()
        .map_or((0..max_queues), |queue| queue..queue + 1)
        .map(|index| {
            let qid = QueueId::new(pass.family(), index);
            let sid = SubmitId::new(qid, families.get_queue(qid).map_or(0, |queue| queue.len()));

            let mut result = Fitness {
                transfers: 0,
                wait_factor: 0,
            };

            // Collect minimal waits required and resource transfers count.
            pass.buffers().for_each(|(id, &state)| {
                let (t, w) = buffers.get(id).map_or((0, 0), |chain| {
                    transfers_and_wait_factor(chain, state, sid, families)
                });
                result.transfers += t;
                result.wait_factor = max(result.wait_factor, w);
            });

            // Collect minimal waits required and resource transfers count.
            pass.images().for_each(|(id, &state)| {
                let (t, w) = images.get(id).map_or((0, 0), |chain| {
                    transfers_and_wait_factor(chain, state, sid, families)
                });
                result.transfers += t;
                result.wait_factor = max(result.wait_factor, w);
            });

            (result, qid)
        })
        .min()
        .unwrap()
}

fn transfers_and_wait_factor<R>(
    chain: &Chain<R>,
    state: State<R>,
    sid: SubmitId,
    families: &Families,
) -> (usize, usize)
where
    R: Resource,
{
    let mut transfers = 0;
    let mut wait_factor = 0;

    let fake_link = Link::new(state, sid);
    if let Some(link) = chain.links().last() {
        transfers += if link.transfer(&fake_link) { 1 } else { 0 };

        for tail in link.tails() {
            wait_factor = max(families[tail].wait_factor, wait_factor);
        }
    }

    (transfers, wait_factor)
}

fn schedule_pass(
    pid: PassId,
    pass: PassDesc,
    qid: QueueId,
    wait_factor: usize,
    families: &mut Families,
    images: &mut ImageChains,
    buffers: &mut BufferChains,
) {
    assert_eq!(qid.family(), pass.family());

    let ref mut queue = families.ensure_queue(qid);
    let sid = queue.add_submit(Submit::new(wait_factor, pid));
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

fn add_to_chain<R>(
    id: Id<R>,
    chain: &mut Chain<R>,
    sid: SubmitId,
    submit: &mut Submit,
    state: State<R>,
) where
    R: Resource,
    Submit: SubmitInsertLink<R>,
{
    let chain_len = chain.links().len();
    let append = match chain.last_link_mut() {
        Some(ref mut link) if link.compatible(state, sid) => {
            submit.insert_link(id, chain_len - 1);
            link.insert_submit(state, sid);
            None
        }
        Some(_) | None => {
            submit.insert_link(id, chain_len);
            Some(Link::new(state, sid))
        }
    };

    if let Some(link) = append {
        chain.add_link(link);
    }
}
