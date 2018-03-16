use std::cmp::max;
use hal::queue::QueueFamilyId;

use chain::{BufferChains, Chain, ImageChains, Link};
use pass::{Pass as PassDesc, PassId};
use resource::{Resource, State};

use resource::Id;
use families::{Families, QueueId, Submit, SubmitId, SubmitInsertLink};

pub struct Schedule {
    pub families: Families,
    pub buffers: BufferChains,
    pub images: ImageChains,
}

#[derive(PartialEq, PartialOrd, Eq, Ord)]
struct Fitness {
    transfers: usize,
    wait_factor: usize,
}

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
        let ((fitness, qid), index) = passes
            .iter()
            .enumerate()
            .filter(|&(_, &(_, ref pass))| all_there(pass.dependencies(), &enqueued))
            .map(|(index, &(_, ref pass))| {
                (
                    fitness(
                        pass,
                        max_queues(pass.family()),
                        &mut images,
                        &mut buffers,
                        &families,
                    ),
                    index,
                )
            })
            .min()
            .unwrap();

        let (index, pass) = passes.swap_remove(index);

        schedule_pass(
            index,
            pass,
            qid,
            fitness,
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

        for semaphore in link.semaphores(&fake_link) {
            wait_factor = max(families[semaphore.submits.start].wait_factor, wait_factor);
        }

        for barrier in link.barriers(&fake_link) {
            wait_factor = max(families[barrier.submits.start].wait_factor, wait_factor);
        }
    }

    (transfers, wait_factor)
}

fn schedule_pass(
    pid: PassId,
    pass: PassDesc,
    qid: QueueId,
    fitness: Fitness,
    families: &mut Families,
    images: &mut ImageChains,
    buffers: &mut BufferChains,
) {
    assert_eq!(qid.family(), pass.family());

    let sid = {
        let ref mut queue = families.ensure_queue(qid);
        queue.add_submit(Submit::new(fitness.wait_factor, pid))
    };

    for (&id, &state) in pass.buffers() {
        let chain = buffers.entry(id).or_insert_with(|| Chain::default());
        append_links(chain, id, sid, state, families);
    }

    for (&id, &state) in pass.images() {
        let chain = images.entry(id).or_insert_with(|| Chain::default());
        append_links(chain, id, sid, state, families);
    }
}

fn append_links<R>(
    chain: &mut Chain<R>,
    id: Id<R>,
    sid: SubmitId,
    state: State<R>,
    families: &mut Families,
) where
    R: Resource,
    Submit: SubmitInsertLink<R>,
{
    let (last_link_index, next_link_index, after_next_link_index) = {
        let chain_len = chain.links().len();
        (chain_len - 1, chain_len, chain_len + 1)
    };

    let append = match chain.last_link_mut() {
        Some(ref mut link) if link.family() != sid.family() => {
            // Different queue families.
            if !link.make_single_queue() {
                // Last link can't release because it is multi-queue.

                // Pick queue.
                let later_sid = link.tails()
                    .into_iter()
                    .max_by_key(|&sid| families[sid].wait_factor)
                    .unwrap();
                let queue_state = link.queue_state(later_sid.queue());

                // Push submit for release.
                let release_sid = {
                    let ref mut queue = families[later_sid.queue()];
                    let wait_factor = queue[later_sid].wait_factor;
                    queue.add_submit(Submit::empty(wait_factor))
                };

                // Release link.
                let release_link = Link::new_single_queue(queue_state, release_sid);

                // Acquire link.
                let acquire_link = Link::new_single_queue(state, sid);

                // Insert release link to release submit
                families[release_sid].insert_link(id, next_link_index);

                // Insert acquire link to target submit
                families[sid].insert_link(id, after_next_link_index);
                vec![release_link, acquire_link]
            } else {
                families[sid].insert_link(id, next_link_index);
                let acquire_link = Link::new_single_queue(state, sid);
                vec![acquire_link]
            }
        }
        Some(ref mut link) if link.compatible(state, sid) => {
            // Compatible with last link.
            families[sid].insert_link(id, last_link_index);
            link.push(state, sid);
            vec![]
        }
        _ => {
            // Incompatible on same family or no links.
            families[sid].insert_link(id, next_link_index);
            vec![Link::new(state, sid)]
        }
    };

    for link in append {
        chain.add_link(link);
    }
}
