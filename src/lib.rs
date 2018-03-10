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
//! 2. Make all passes declare their `PassLinks`.
//! 3. Build `ResourceChainSet` from all `PassLinks`.
//! 

#![deny(missing_docs)]
#![deny(unused_must_use)]
#![deny(dead_code)]

#[macro_use]
extern crate derivative;
extern crate gfx_hal as hal;

#[macro_use]
extern crate log;


/// Implement `Resource` for buffer type.
/// Buffer type must implement `Borrow<B::Buffer>`.
/// Otherwise borrowing function must be provided.
#[macro_export]
macro_rules! impl_buffer_resource {
    ($backend:ty, $buffer:ty) => {
        impl_buffer_resource!($backend, $buffer, Borrow::borrow);
    };

    ($backend:ty, $buffer:ty, $borrow:expr) => {
        impl Resource for $buffer {
            type Backend = $backend;
            type Access = BufferAccess;
            type Layout = BufferLayout;
            type Usage = BufferUsage;
            type Range = Range<Option<u64>>;

            fn big_barrier<'a>(access: Range<BufferAccess>) -> Barrier<'a, B> {
                Barrier::AllBuffers(access)
            }

            fn barrier<'a>(
                &'a self,
                access: Range<BufferAccess>,
                _layout: Range<BufferLayout>,
                _range: Range<Option<u64>>,
            ) -> Barrier<'a, B> {
                Barrier::Buffer {
                    states: access,
                    target: borrow(self),
                }
            }
        }
    };
}

/// Implement `Resource` for image type.
/// Image type must implement `Borrow<B::Image>`.
/// Otherwise borrowing function must be provided.
#[macro_export]
macro_rules! impl_image_resource {
    ($backend:ty, $image:ty) => {
        impl_image_resource!($backend, $image, Borrow::borrow);
    };

    ($backend:ty, $image:ty, $borrow:expr) => {
        impl<B, T> $crate::Resource for $image {
            type Backend = $backend;
            type Access = ImageAccess;
            type Layout = ImageLayout;
            type Usage = ImageUsage;
            type Range = SubresourceRange;

            fn big_barrier<'a>(access: Range<ImageAccess>) -> Barrier<'a, B> {
                Barrier::AllImages(access)
            }

            fn barrier<'a>(
                &'a self,
                access: Range<ImageAccess>,
                layout: Range<ImageLayout>,
                range: SubresourceRange,
            ) -> Barrier<'a, B> {
                Barrier::Image {
                    states: (access.start, layout.start)..(access.end, layout.end),
                    range,
                    target: borrow(self),
                }
            }
        }
    };
}

mod buffer;
mod chain;
mod image;
mod queue;
mod resource;

pub use buffer::{BufferLayout, BufferChain, BufferChainSet};
pub use chain::{ChainId, Link, PassLink, PassLinks, Chain, ChainSet};
pub use image::{ImageChain, ImageChainSet};
pub use queue::QueueId;
pub use resource::{Access, Layout, Resource, Usage};
