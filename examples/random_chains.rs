extern crate clap;
extern crate gfx_hal as hal;
extern crate gfx_chain;
extern crate rand;

use clap::{Arg, App, SubCommand};
use gfx_chain::chain::Chain;
use gfx_chain::collect::{Chains, collect};
use gfx_chain::pass::{Pass, PassId, StateUsage};
use gfx_chain::resource::{Buffer, BufferLayout, State, Id, Image, Resource, Usage, Layout};
use gfx_chain::schedule::{QueueId, SubmissionId};
use gfx_chain::sync::{SyncData, Barrier, sync};
use hal::buffer::{Access as BufferAccess};
use hal::image::{Access as ImageAccess, Layout as ImageLayout};
use hal::pso::PipelineStage;
use hal::queue::QueueFamilyId;
use rand::{Rng, SeedableRng, OsRng, Isaac64Rng};
use std::cmp::min;
use std::collections::{HashMap, HashSet};
use std::panic::{catch_unwind, set_hook, AssertUnwindSafe};
use std::time::{Instant, Duration};

type DefaultRng = Isaac64Rng;

fn gen_bool(rng: &mut DefaultRng) -> bool {
    rng.gen_range(0, 2) == 0
}
fn gen_inclusive_u32(rng: &mut DefaultRng, low: u32, high: u32) -> u32 {
    rng.gen_range(low, high + 1)
}
fn gen_inclusive(rng: &mut DefaultRng, low: usize, high: usize) -> usize {
    rng.gen_range(low, high + 1)
}
fn gen_inclusive_prefer_low(rng: &mut DefaultRng, low: usize, high: usize) -> usize {
    if gen_bool(rng) {
        rng.gen_range(low, high + 1)
    } else {
        low
    }
}

fn create_buffer_access_single(rng: &mut DefaultRng) -> BufferAccess {
    *rng.choose(&[
        BufferAccess::INDIRECT_COMMAND_READ,
        BufferAccess::INDEX_BUFFER_READ,
        BufferAccess::VERTEX_BUFFER_READ,
        BufferAccess::CONSTANT_BUFFER_READ,
        BufferAccess::SHADER_READ,
        BufferAccess::SHADER_WRITE,
        BufferAccess::TRANSFER_READ,
        BufferAccess::TRANSFER_WRITE,
        BufferAccess::HOST_READ,
        BufferAccess::HOST_WRITE,
        BufferAccess::MEMORY_READ,
        BufferAccess::MEMORY_WRITE,
      ]).unwrap()
}
fn create_buffer_access(rng: &mut DefaultRng) -> BufferAccess {
    let mut access = BufferAccess::empty();
    for _ in 0..gen_inclusive_prefer_low(rng, 1, 4) {
        access |= create_buffer_access_single(rng);
    }
    access
}

fn create_image_access_single(rng: &mut DefaultRng) -> ImageAccess {
    *rng.choose(&[
        ImageAccess::INPUT_ATTACHMENT_READ,
        ImageAccess::SHADER_READ,
        ImageAccess::SHADER_WRITE,
        ImageAccess::COLOR_ATTACHMENT_READ,
        ImageAccess::COLOR_ATTACHMENT_WRITE,
        ImageAccess::DEPTH_STENCIL_ATTACHMENT_READ,
        ImageAccess::DEPTH_STENCIL_ATTACHMENT_WRITE,
        ImageAccess::TRANSFER_READ,
        ImageAccess::TRANSFER_WRITE,
        ImageAccess::HOST_READ,
        ImageAccess::HOST_WRITE,
        ImageAccess::MEMORY_READ,
        ImageAccess::MEMORY_WRITE,
    ]).unwrap()
}
fn create_image_access(rng: &mut DefaultRng) -> ImageAccess {
    let mut access = ImageAccess::empty();
    for _ in 0..gen_inclusive_prefer_low(rng, 1, 4) {
        access |= create_image_access_single(rng);
    }
    access
}

fn create_image_layout(rng: &mut DefaultRng) -> ImageLayout {
    *rng.choose(&[
        ImageLayout::General,
        ImageLayout::ColorAttachmentOptimal,
        ImageLayout::DepthStencilAttachmentOptimal,
        ImageLayout::DepthStencilReadOnlyOptimal,
        ImageLayout::ShaderReadOnlyOptimal,
        ImageLayout::TransferSrcOptimal,
        ImageLayout::TransferDstOptimal,
        ImageLayout::Present,
    ]).unwrap()
}

fn create_pipeline_stage_single(rng: &mut DefaultRng) -> PipelineStage {
    *rng.choose(&[PipelineStage::DRAW_INDIRECT,
        PipelineStage::VERTEX_INPUT,
        PipelineStage::VERTEX_SHADER,
        PipelineStage::HULL_SHADER,
        PipelineStage::DOMAIN_SHADER,
        PipelineStage::GEOMETRY_SHADER,
        PipelineStage::FRAGMENT_SHADER,
        PipelineStage::EARLY_FRAGMENT_TESTS,
        PipelineStage::LATE_FRAGMENT_TESTS,
        PipelineStage::COLOR_ATTACHMENT_OUTPUT,
        PipelineStage::COMPUTE_SHADER,
        PipelineStage::TRANSFER,
    ]).unwrap()
}
fn create_pipeline_stage(rng: &mut DefaultRng) -> PipelineStage {
    let mut stage = PipelineStage::empty();
    for _ in 0..gen_inclusive_prefer_low(rng, 1, 4) {
        stage |= create_pipeline_stage_single(rng);
    }
    stage
}

fn create_buffer_state(rng: &mut DefaultRng) -> State<Buffer> {
    State {
        access: create_buffer_access(rng),
        layout: BufferLayout,
        stages: create_pipeline_stage(rng),
    }
}
fn create_image_state(rng: &mut DefaultRng) -> State<Image> {
    State {
        access: create_image_access(rng),
        layout: create_image_layout(rng),
        stages: create_pipeline_stage(rng),
    }
}

fn create_deps(rng: &mut DefaultRng, i: usize) -> Vec<PassId> {
    let max_deps = min(i, 5);
    let mut deps = Vec::new();
    for _ in 0..gen_inclusive(rng, 0, max_deps) {
        let pass_id = PassId(rng.gen_range(0, i));
        if !deps.contains(&pass_id) {
            deps.push(pass_id);
        }
    }
    deps
}

fn create_resc_deps<R: Resource, F: Fn(&mut DefaultRng) -> State<R>>(
    rng: &mut DefaultRng, count: u32, used: &mut HashSet<Id<R>>, new_state: F,
) -> HashMap<Id<R>, StateUsage<R>> {
    let mut map = HashMap::new();
    if count != 0 {
        for _ in 0..gen_inclusive_u32(rng, 1, min(count, 4)) {
            let id = Id::new(rng.gen_range(0, count));
            used.insert(id);
            map.insert(id, StateUsage { state: new_state(rng), usage: R::Usage::none() });
        }
    }
    map
}

static mut PANIC_INFO: Option<Option<String>> = None;
fn install_fuzz_panic_hook() {
    set_hook(Box::new(|panic_info| {
        if let Some(location) = panic_info.location() {
            unsafe { PANIC_INFO = Some(Some(format!("{}:{}", location.file(), location.line()))) }
        } else {
            unsafe { PANIC_INFO = Some(None) }
        }
    }));
}
fn take_panic_info() -> Option<Option<String>> {
    unsafe { PANIC_INFO.take() }
}

// TODO: Verify pipeline stages are sane.

fn fill<T: Default>(num: usize) -> Vec<T> {
    let mut vec = Vec::with_capacity(num);
    for _ in 0..num {
        vec.push(T::default());
    }
    vec
}

#[derive(Copy, Clone, Debug)]
enum ExecuteTarget {
    Acquire(usize),
    Pass(usize),
    Release(usize),
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
enum QueueStage {
    BeforeAcquire(usize),
    BeforePass(usize),
    BeforeRelease(usize),
    AllExecuted,
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
enum ResourceOwner {
    OnQueue(QueueFamilyId),
    TransferringTo(QueueFamilyId),
}

#[derive(Copy, Clone, Debug)]
struct ResourceState<R: Resource> {
    access: R::Access,
    layout: R::Layout,
    owner : ResourceOwner,
}

struct ExecuteStatus<'a, 'b> {
    chains: &'a Chains<SyncData<usize, usize>>,
    passes: &'b Vec<Pass>,
    log: bool,
    queue_state: HashMap<QueueId, QueueStage>,
    image_state: HashMap<Id<Image>, ResourceState<Image>>,
    buffer_state: HashMap<Id<Buffer>, ResourceState<Buffer>>,
    completed_passes: Vec<bool>,
    signaled_semaphores: Vec<bool>,
}
impl <'a, 'b> ExecuteStatus<'a, 'b> {
    pub fn new(
        chains: &'a Chains<SyncData<usize, usize>>, passes: &'b Vec<Pass>,
        semaphore_count: usize, log: bool,
    ) -> Self {
        if log {
            println!("Test executing schedule...")
        }

        let mut queue_state = HashMap::new();
        for queue in chains.schedule.iter().flat_map(|family| family.iter()) {
            let qid = queue.id();
            queue_state.insert(qid, if queue.len() == 0 {
                QueueStage::AllExecuted
            } else {
                QueueStage::BeforeAcquire(0)
            });
        }

        let mut buffer_state = HashMap::new();
        for (&id, buffer) in &chains.buffers {
            let link = buffer.link(0);
            buffer_state.insert(id, ResourceState {
                access: link.state().access,
                layout: link.state().layout,
                owner: ResourceOwner::OnQueue(link.family()),
            });
        }

        let mut image_state = HashMap::new();
        for (&id, image) in &chains.images {
            let link = image.link(0);
            image_state.insert(id, ResourceState {
                access: link.state().access,
                layout: link.state().layout,
                owner: ResourceOwner::OnQueue(link.family()),
            });
        }

        let completed_passes = fill(passes.len());
        let signaled_semaphores = fill(semaphore_count);

        ExecuteStatus {
            chains, passes, log, queue_state,
            image_state, buffer_state,
            completed_passes, signaled_semaphores,
        }
    }

    fn barrier_new_state<R: Resource>(
        current_family: QueueFamilyId, barrier: &Barrier<R>, old_state: ResourceState<R>,
    ) -> ResourceState<R> {
        let mut new_state = old_state;
        if let Some(ref transfer) = barrier.queues {
            assert_ne!(transfer.start, transfer.end, "Transfer from family to itself!");
            if transfer.start.family() == current_family {
                assert_eq!(new_state.owner, ResourceOwner::OnQueue(current_family),
                           "Attempting to transfer resource out from queue that doesn't own it.");
                new_state.owner = ResourceOwner::TransferringTo(transfer.end.family());
            } else if transfer.end.family() == current_family {
                assert_eq!(new_state.owner, ResourceOwner::TransferringTo(transfer.end.family()),
                           "Attempting to transfer resource in without related transfer out.");
                new_state.owner = ResourceOwner::OnQueue(transfer.end.family());
            } else {
                panic!("Attempt to transfer resource from unrelated queue.");
            }
        }

        if let ResourceOwner::OnQueue(_) = old_state.owner {
            // TODO: Handle source = Undefined for images
            assert_eq!(barrier.states.start.layout, old_state.layout,
                       "Resource source layout does not match actual resource layout.");
            new_state.layout = barrier.states.end.layout;
        }

        // TODO: Check that the transition done by the transfer out matches the transfer in

        assert_eq!(barrier.states.start.access, old_state.access,
                   "Resource source access flags do not match actual resource access.");
        new_state.access = barrier.states.end.access;

        new_state
    }
    fn execute_barrier<R: Resource>(
        map: &mut HashMap<Id<R>, ResourceState<R>>,
        current_family: QueueFamilyId, id: Id<R>, barrier: &Barrier<R>,
    ) {
        let old_state = *map.get(&id).expect("Resource not in chain!");
        let new_state = Self::barrier_new_state(current_family, barrier, old_state);
        map.insert(id, new_state);
    }
    fn can_execute_guard(&self, sid: SubmissionId, is_release: bool) -> bool {
        let sync = self.chains.schedule.submission(sid).expect("Submission does not exist?");
        let guard = if is_release { &sync.sync().release } else { &sync.sync().acquire };
        guard.wait.iter().all(|wait| self.signaled_semaphores[*wait.semaphore()])
    }
    fn execute_guard(&mut self, sid: SubmissionId, is_release: bool) {
        assert!(self.can_execute_guard(sid, is_release));

        let sub = self.chains.schedule.submission(sid).expect("Submission does not exist?");
        let guard = if is_release { &sub.sync().release } else { &sub.sync().acquire };
        let pass_data = &self.passes[sub.pass().0];

        if self.log {
            println!(" - Executing {} guard for {:?} as {:?}",
                     if is_release { "release" } else { "acquire" }, pass_data.id, sid);
        }

        for (&id, barrier) in &guard.buffers {
            Self::execute_barrier(&mut self.buffer_state, sid.family(), id, barrier);
        }
        for (&id, barrier) in &guard.images {
            Self::execute_barrier(&mut self.image_state, sid.family(), id, barrier);
        }

        for signal in &guard.signal {
            let id = *signal.semaphore();
            assert!(!self.signaled_semaphores[id], "Semaphore already signaled.");
            self.signaled_semaphores[id] = true;
        }
    }

    fn check_pass_state<R: Resource>(
        map: &HashMap<Id<R>, ResourceState<R>>, chains: &HashMap<Id<R>, Chain<R>>,
        current_family: QueueFamilyId, id: Id<R>, expected_state: StateUsage<R>, link_id: usize,
    ) {
        let state = *map.get(&id).expect("Resource not in chain!");
        let link = chains.get(&id).expect("Resource not in chain!").link(link_id);

        assert_eq!(state.owner, ResourceOwner::OnQueue(current_family),
                   "Resource is not currently owned by the queue executing this pass.");
        assert_eq!(state.layout, link.state().layout,
                   "Current layout does not match layout specified in chain.");
        assert!(state.layout.merge(expected_state.state.layout).is_some(),
                "Current layout is not compatible with expected layout.");
        assert_eq!(state.layout, link.state().layout,
                   "Current access flags do not match flags specified in chain.");
        assert_eq!(state.access & expected_state.state.access, expected_state.state.access,
                   "Current access flags do not contain all expected access flags.");
    }
    fn execute_pass(&mut self, sid: SubmissionId) {
        let sub = self.chains.schedule.submission(sid).expect("Submission does not exist?");
        let pass_data = &self.passes[sub.pass().0];

        if self.log {
            println!(" - Executing main pass for {:?} as {:?}", pass_data.id, sid);
        }

        // TODO: Figure out what's reasonable to do with multiple queues.
        // (This is too strong a restriction, chains doesn't guarantee this much.)
        if self.queue_state.len() == 1 {
            for dep in &pass_data.dependencies {
                assert!(self.completed_passes[dep.0], "Dependant executed before dependency!");
            }
        }
        for (&id, &state) in &pass_data.buffers {
            Self::check_pass_state(&self.buffer_state, &self.chains.buffers,
                                   sid.family(), id, state, sub.buffer(id));
        }
        for (&id, &state) in &pass_data.images {
            Self::check_pass_state(&self.image_state, &self.chains.images,
                                   sid.family(), id, state, sub.image(id));
        }
        self.completed_passes[sub.pass().0] = true;
    }

    fn can_execute(&self, qid: QueueId, target: ExecuteTarget) -> bool {
        match target {
            ExecuteTarget::Acquire(index) =>
                self.can_execute_guard(SubmissionId::new(qid, index), false),
            ExecuteTarget::Pass(_) =>
                true,
            ExecuteTarget::Release(index) =>
                self.can_execute_guard(SubmissionId::new(qid, index), true),
        }
    }
    fn execute(&mut self, qid: QueueId, target: ExecuteTarget) {
        match target {
            ExecuteTarget::Acquire(index) =>
                self.execute_guard(SubmissionId::new(qid, index), false),
            ExecuteTarget::Pass(index) =>
                self.execute_pass(SubmissionId::new(qid, index)),
            ExecuteTarget::Release(index) =>
                self.execute_guard(SubmissionId::new(qid, index), true),
        }
    }

    fn advance_queue(
        queue_length: usize, stage: QueueStage,
    ) -> Option<(ExecuteTarget, QueueStage)> {
        if queue_length == 0 {
            return None
        }
        match stage {
            QueueStage::BeforeAcquire(index) =>
                Some((ExecuteTarget::Acquire(index), QueueStage::BeforePass(index))),
            QueueStage::BeforePass(index) =>
                Some((ExecuteTarget::Pass(index), QueueStage::BeforeRelease(index))),
            QueueStage::BeforeRelease(index) => if index == queue_length - 1 {
                Some((ExecuteTarget::Release(index), QueueStage::AllExecuted))
            } else {
                Some((ExecuteTarget::Release(index), QueueStage::BeforeAcquire(index + 1)))
            },
            QueueStage::AllExecuted =>
                None,
        }
    }

    fn is_finished(&self) -> bool {
        self.queue_state.values().all(|stage| *stage == QueueStage::AllExecuted)
    }
    fn execute_queue(&mut self, qid: QueueId) -> bool {
        let queue_len =
            self.chains.schedule.queue(qid).expect("Expected queue not in schedule.").len();
        let stage = *self.queue_state.get(&qid).expect("Unknown queue?");
        if let Some((execute_target, next_stage)) = Self::advance_queue(queue_len, stage) {
            if self.can_execute(qid, execute_target) {
                self.execute(qid, execute_target);
                self.queue_state.insert(qid, next_stage);
                return true
            }
        }
        false
    }
    fn execute_random(&mut self, rng: &mut DefaultRng) {
        let mut queues: Vec<_> = self.queue_state.keys().map(|x| *x).collect();
        queues.sort();
        rng.shuffle(&mut queues);
        for queue in queues {
            if self.execute_queue(queue) {
                return
            }
        }
        panic!("No queue could be executed.")
    }
    fn execute_all(mut self, rng: &mut DefaultRng) {
        while !self.is_finished() {
            self.execute_random(rng)
        }
    }
}

fn sanity_check(
    rng: &mut DefaultRng,
    chains: &Chains<SyncData<usize, usize>>, passes: &Vec<Pass>, semaphore_count: usize,
    log: bool,
) {
    ExecuteStatus::new(chains, passes, semaphore_count, log).execute_all(rng)
}

#[derive(Copy, Clone)]
struct BenchParams {
    family_count: usize, queues_per_family: usize,
    buffer_count: u32, image_count: u32, submit_count: usize,
}

fn test_run(seed: u64, is_test: bool, bench: Option<BenchParams>) -> (usize, Duration) {
    if is_test {
        println!("Seed: {}", seed);
    }

    let mut rng = Isaac64Rng::new_unseeded();
    rng.reseed(&[seed]);
    let rng = &mut rng;

    let (family_count, buffer_count, image_count, submit_count) = if let Some(bench) = bench {
        (bench.family_count, bench.buffer_count, bench.image_count, bench.submit_count)
    } else {
        (gen_inclusive_prefer_low(rng, 1, 4),
         gen_inclusive_u32(rng, 0, 10),
         gen_inclusive_u32(rng, 0, 10),
         gen_inclusive(rng, 1, 25))
    };

    if is_test {
        println!("Creating test case with {} families, {} buffers, {} images, and {} submissions.",
                 family_count, buffer_count, image_count, submit_count);
    }

    let mut max_queues = Vec::new();
    for _ in 0..family_count {
        if let Some(bench) = bench {
            max_queues.push(bench.queues_per_family)
        } else {
            max_queues.push(gen_inclusive_prefer_low(rng, 1, 5));
        }
    }
    if is_test {
        println!("Max queues for families: {:?}", max_queues);
    }

    let mut used_families = HashSet::new();
    let mut used_buffers = HashSet::new();
    let mut used_images = HashSet::new();

    let mut passes = Vec::new();
    let mut pass_complexity = 0;
    for i in 0..submit_count {
        let family = QueueFamilyId(rng.gen_range(0, family_count));
        let queue = if gen_bool(rng) { Some(rng.gen_range(0, max_queues[family.0])) } else { None };
        let dependencies = create_deps(rng, i);
        let buffers = create_resc_deps(rng, buffer_count, &mut used_buffers, create_buffer_state);
        let images = create_resc_deps(rng, image_count, &mut used_images, create_image_state);

        used_families.insert(family);
        if queue.is_some() { pass_complexity += 1; }
        pass_complexity += dependencies.len() + buffers.len() + images.len();

        passes.push(Pass {
            id: PassId(i), family, queue, dependencies, buffers, images,
        })
    }
    if is_test {
        println!("Submissions: {:#?}", passes);
    }

    let mut shuffled_passes = passes.clone();
    rng.shuffle(&mut shuffled_passes);

    let now = Instant::now();
    catch_unwind(AssertUnwindSafe(|| {
        let chains = collect(shuffled_passes, |QueueFamilyId(id)| max_queues[id]);
        if is_test {
            println!("Unsynched chains: {:#?}", chains);
        }

        let mut semaphore_id = 0;
        let schedule = sync(&chains, || {
            let id = semaphore_id;
            semaphore_id += 1;
            (id, id)
        });

        if is_test {
            println!("Schedule: {:#?}", schedule);
            println!("Semaphore count: {}", semaphore_id);
        }

        let synched_chains = Chains {
            schedule, buffers: chains.buffers, images: chains.images,
        };
        for _ in 0..10 {
            sanity_check(rng, &synched_chains, &passes, semaphore_id, is_test);
        }
    })).ok();
    let duration = Instant::now().duration_since(now);

    let mut queue_count = 0;
    for &family in &used_families {
        queue_count += max_queues[family.0];
    }

    let queue_score = (used_families.len() - 1) * 10 + (queue_count - 1) as usize;
    let object_score = submit_count + used_buffers.len() + used_images.len();
    (queue_score * 100 + object_score * 10 + pass_complexity, duration)
}
fn run_bench(os_rng: &mut OsRng, bench: &str, params: BenchParams) {
    let now = Instant::now();
    let mut total_ms: f64 = 0.0;
    let mut total_ms_sq: f64 = 0.0;
    let mut iters = 0;
    while Instant::now().duration_since(now).as_secs() <= 3 {
        let seed = os_rng.next_u64();
        let (_, run_time) = test_run(seed, false, Some(params));
        let ms = run_time.subsec_nanos() as f64 / 1000000.0 + run_time.as_secs() as f64 * 1000.0;
        total_ms += ms;
        total_ms_sq += ms * ms;
        iters += 1;
    }

    let mean_ms = total_ms / iters as f64;
    let variance_ms = (total_ms_sq / iters as f64) - mean_ms * mean_ms;
    let std_ms = variance_ms.abs().sqrt();
    println!("{}: {:-6} iters ran in average {:.3} ± {:.3} ms ({:.2}% ± {:.2}% of 16.6 ms)",
             bench, iters,
             mean_ms, std_ms,
             mean_ms / 16.6 * 100.0, std_ms / 16.6 * 100.0);
}

fn main() {
    let mut app = App::new("gfx-chains random tester")
                           .subcommand(SubCommand::with_name("test")
                                       .about("Tests a random chain, printing all results.")
                                       .arg(Arg::with_name("SEED")
                                            .help("optional fixed seed").index(1)))
                           .subcommand(SubCommand::with_name("bench")
                                       .about("Benchmarks large random chains."))
                           .subcommand(SubCommand::with_name("fuzz")
                                       .about("Tests random chains, to find panicking cases."));
    let matches = app.clone().get_matches();

    let mut os_rng = OsRng::new().unwrap();

    if let Some(matches) = matches.subcommand_matches("test") {
        match matches.value_of("SEED") {
            Some(seed) => {
                let seed = seed.parse::<u64>().expect("seed is not a number!");
                test_run(seed, true, None);
            }
            None => {
                test_run(os_rng.next_u64(), true, None);
            }
        }
        return
    }
    if let Some(_) = matches.subcommand_matches("bench") {
        for &(load_name, resc_count, submit_count) in &[
            ("tiny  ", 2, 10),
            ("small ", 10, 50),
            ("medium", 15, 150),
            ("large ", 50, 1000),
            ("huge  ", 50, 1500),
        ] {
            for &(queue_name, queue_count) in &[
                ("single-queue", 1),
                ("multi-queue ", 3)
            ] {
                run_bench(&mut os_rng, &format!("{} + {}", load_name, queue_name), BenchParams {
                    family_count: queue_count, queues_per_family: queue_count,
                    buffer_count: resc_count, image_count: resc_count, submit_count,
                });
            }
        }
        return
    }
    if let Some(_) = matches.subcommand_matches("fuzz") {
        install_fuzz_panic_hook();
        let mut simplest_examples = HashMap::new();
        loop {
            let seed = os_rng.next_u64();
            let (example_complexity, _) = test_run(seed, false, None);
            match take_panic_info() {
                Some(Some(location)) => {
                    if simplest_examples.contains_key(&location) {
                        let entry = simplest_examples.get_mut(&location).unwrap();
                        if example_complexity < *entry {
                            println!("Simpler example found for panic at {}. \
                                      Seed = {}, Example complexity = {}",
                                     location, seed, example_complexity);
                            *entry = example_complexity;
                        }
                    } else {
                        println!("/!\\ New panic found at {}. Seed = {}, Example complexity = {}",
                                 location, seed, example_complexity);
                        simplest_examples.insert(location, example_complexity);
                    }
                }
                Some(None) => {
                    println!("/!\\ New panic found at unknown location. Seed = {}, \
                              Example complexity = {}", seed, example_complexity);
                }
                None => { }
            }
        }
    }
    app.print_help().unwrap();
    println!();
}