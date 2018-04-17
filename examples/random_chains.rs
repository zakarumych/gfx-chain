extern crate clap;
extern crate gfx_hal as hal;
extern crate gfx_chain;
extern crate rand;

use clap::{Arg, App, SubCommand};
use gfx_chain::build;
use gfx_chain::pass::{Pass, PassId};
use gfx_chain::resource::{Buffer, BufferLayout, State, Id, Image};
use hal::buffer::{Access as BufferAccess};
use hal::image::{Access as ImageAccess, Layout as ImageLayout};
use hal::pso::PipelineStage;
use hal::queue::QueueFamilyId;
use rand::{Rng, SeedableRng, OsRng, Isaac64Rng};
use std::cmp::min;
use std::collections::HashMap;
use std::panic::{catch_unwind, set_hook, AssertUnwindSafe};

type DefaultRng = Isaac64Rng;

fn gen_inclusive_u32(rng: &mut DefaultRng, low: u32, high: u32) -> u32 {
    rng.gen_range(low, high + 1)
}
fn gen_inclusive(rng: &mut DefaultRng, low: usize, high: usize) -> usize {
    rng.gen_range(low, high + 1)
}
fn gen_inclusive_prefer_low(rng: &mut DefaultRng, low: usize, high: usize) -> usize {
    if rng.gen_range(0, 2) == 0 {
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

fn create_buffer_deps(rng: &mut DefaultRng, count: u32) -> HashMap<Id<Buffer>, State<Buffer>> {
    let mut map = HashMap::new();
    for _ in 0..gen_inclusive_u32(rng, 1, min(count, 4)) {
        map.insert(Id::new(rng.gen_range(0, count)), create_buffer_state(rng));
    }
    map
}
fn create_image_deps(rng: &mut DefaultRng, count: u32) -> HashMap<Id<Image>, State<Image>> {
    let mut map = HashMap::new();
    for _ in 0..gen_inclusive_u32(rng, 1, min(count, 4)) {
        map.insert(Id::new(rng.gen_range(0, count)), create_image_state(rng));
    }
    map
}

fn test_run(seed: u64, debug_print: bool) {
    if debug_print {
        println!("Seed: {}", seed);
    }

    let mut rng = Isaac64Rng::new_unseeded();
    rng.reseed(&[seed]);
    let rng = &mut rng;

    let queue_count = gen_inclusive_prefer_low(rng, 1, 4);
    let buffer_count = gen_inclusive_u32(rng, 1, 10);
    let image_count = gen_inclusive_u32(rng, 1, 10);
    let submit_count = gen_inclusive(rng, 10, 30);
    if debug_print {
        println!("Creating test case with {} queues, {} buffers, {} images, and {} submissions.",
                 queue_count, buffer_count, image_count, submit_count);
    }

    let mut max_queues = Vec::new();
    for _ in 0..queue_count {
        max_queues.push(gen_inclusive_prefer_low(rng, 1, 5));
    }
    if debug_print {
        println!("Max queues for families: {:?}", max_queues);
    }

    let mut passes = Vec::new();
    for i in 0..submit_count {
        passes.push(Pass {
            id: PassId(i),
            family: QueueFamilyId(rng.gen_range(0, queue_count)),
            queue: None,
            dependencies: create_deps(rng, i),
            buffers: create_buffer_deps(rng, buffer_count),
            images: create_image_deps(rng, image_count),
        })
    }
    if debug_print {
        println!("Submissions: {:#?}", passes);
    }

    let mut semaphore_id = 0;
    let chains = build(
        passes,
        |QueueFamilyId(id)| max_queues[id],
        || {
            let id = semaphore_id;
            semaphore_id += 1;
            (id, id)
        }
    );

    if debug_print {
        println!("Schedule: {:#?}", chains.schedule);
        println!("Semaphore count: {}", semaphore_id);
    }
}

fn main() {
    let mut app = App::new("gfx-chains random tester")
                           .subcommand(SubCommand::with_name("test")
                                       .about("Tests a random chain, printing all results.")
                                       .arg(Arg::with_name("SEED")
                                            .help("optional fixed seed").index(1)))
                           .subcommand(SubCommand::with_name("fuzz")
                                       .about("Tests random chains, to find panicing cases."));
    let matches = app.clone().get_matches();

    let mut os_rng = OsRng::new().unwrap();

    if let Some(matches) = matches.subcommand_matches("test") {
        match matches.value_of("SEED") {
            Some(seed) => {
                let seed = seed.parse::<u64>().expect("seed is not a number!");
                test_run(seed, true);
            }
            None => test_run(os_rng.next_u64(), true),
        }
        return
    }
    if let Some(_) = matches.subcommand_matches("fuzz") {
        static mut SEED: u64 = 0;
        set_hook(Box::new(|panic_info| {
            if let Some(location) = panic_info.location() {
                println!("Panic occurred: seed = {}, location = {}:{}",
                         unsafe { SEED }, location.file(), location.line());
            } else {
                println!("Panic occurred: seed = {}, location = <unknown>", unsafe { SEED });
            }
        }));
        loop {
            let seed = os_rng.next_u64();
            unsafe { SEED = seed; }
            catch_unwind(AssertUnwindSafe(|| test_run(seed, false))).ok();
        }
    }
    app.print_help().unwrap();
    println!();
}