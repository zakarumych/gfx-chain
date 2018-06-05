//!
//! This crates helps the users of low-level graphics API of `gfx-hal` to reason about
//! what passes uses what resources and how those resources are accessed.
//! With this information `gfx-chain` may automatically derive synchronization commands required
//! before and after each pass.
//!

#![deny(missing_docs)]
#![deny(unused_must_use)]
#![deny(dead_code)]

extern crate fnv;
extern crate gfx_hal as hal;

use hal::queue::QueueFamilyId;

pub mod chain;
pub mod collect;
pub mod pass;
pub mod resource;
pub mod schedule;
pub mod sync;

/// Allows to insert links to submission generically.
trait Pick<R> {
    type Target;
    fn pick(&self) -> &Self::Target;
    fn pick_mut(&mut self) -> &mut Self::Target;
}

use pass::Pass;
use collect::{Chains, collect};
use sync::{sync, SyncData};

/// Build synchronized schedule of the execution from passes descriptions.
/// 
/// # Parameters
/// 
/// `passes`        - array of pass descriptions for passes to schedule and synchronize.
/// `max_queues`    - function that returns maximum number of queues for specified family.
/// `new_semaphore` - function to create new semaphore pair - (signal, wait).
/// 
pub fn build<F, Q, S, W>(passes: Vec<Pass>, max_queues: Q, new_semaphore: F) -> Chains<SyncData<S, W>>
where
    Q: Fn(QueueFamilyId) -> usize,
    F: FnMut() -> (S, W),
{
    let chains = collect(passes, max_queues);
    let schedule = sync(&chains, new_semaphore);
    Chains {
        schedule,
        images: chains.images,
        buffers: chains.buffers,
    }
}
