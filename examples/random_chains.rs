extern crate clap;
extern crate gfx_hal as hal;
extern crate gfx_chain;
extern crate rand;

use clap::{Arg, App, SubCommand};
use gfx_chain::collect::collect;
use gfx_chain::pass::{Pass, PassId, StateUsage};
use gfx_chain::resource::{Buffer, BufferLayout, State, Id, Image, Resource, Usage};
use gfx_chain::sync::sync;
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
        //ImageLayout::Present, // FIXME
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
    rng.shuffle(&mut passes);

    let now = Instant::now();
    catch_unwind(AssertUnwindSafe(|| {
        let chains = collect(passes, |QueueFamilyId(id)| max_queues[id]);
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