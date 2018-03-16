//!
//! This crates helps the users of low-level graphics API of `gfx-hal` to reason about
//! what passes uses what resources and how those resources are accessed.
//! With this information `gfx-chain` may automatically derive synchronization commands required
//! before and after each pass.
//!
//! In order to start working with `gfx-chain` users must complete a few steps.
//! 1. Implement `Resource` for their resource wrappers.
//!    To do so a pair of macros are provided.
//!
//! 2. Make all passes declare their `PassesDesc`.
//! 3. Build `ResourceChainSet` from all `PassesDesc`.
//!

// #![deny(missing_docs)]
#![deny(unused_must_use)]
// #![deny(dead_code)]

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
