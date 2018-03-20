//!
//! This crates helps the users of low-level graphics API of `gfx-hal` to reason about
//! what passes uses what resources and how those resources are accessed.
//! With this information `gfx-chain` may automatically derive synchronization commands required
//! before and after each pass.
//!

#![deny(missing_docs)]
#![deny(unused_must_use)]
#![deny(dead_code)]

#[macro_use]
extern crate derivative;
extern crate gfx_hal as hal;

#[macro_use]
extern crate log;

pub mod resource;
pub mod pass;
pub mod families;
pub mod chain;
pub mod schedule;
pub mod sync;

/// Allows to insert links to submit generically.
pub(crate) trait Pick<R> {
    type Target;
    fn pick(&self) -> &Self::Target;
    fn pick_mut(&mut self) -> &mut Self::Target;
}
