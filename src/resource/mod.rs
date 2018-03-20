//!
//! This module provides `Resource` trait and a pair of implementations: `Buffer` and `Image`.
//! `Resource` trait together with `Access`, `Layout` and `Usage` allows user to deal with resource states more generically.
//!

mod access;
mod buffer;
mod image;
mod layout;
mod usage;

use std::fmt::Debug;
use std::hash::Hash;
use std::marker::PhantomData;
use std::ops::Range;
use hal::buffer::{Access as BufferAccess, Usage as BufferUsage};
use hal::image::{Access as ImageAccess, ImageLayout, SubresourceRange, Usage as ImageUsage};
use hal::pso::PipelineStage;

pub use self::access::Access;
pub use self::buffer::BufferLayout;
pub use self::layout::Layout;
pub use self::usage::Usage;

/// Defines resource type.
/// Should be implemented for buffers and images.
pub trait Resource: Copy + Debug + Eq + Ord + Hash {
    /// Access type of the resource.
    type Access: Access;

    /// Layout type of the resource.
    type Layout: Layout;

    /// Usage type of the resource.
    type Usage: Usage;

    /// Sub-resource range.
    type Range: Clone;

    /// Resource type index.
    const ID: u32;
}

/// Buffer resource.
/// Implements `Resource` with associated types required for buffers.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Buffer {}
impl Resource for Buffer {
    type Access = BufferAccess;
    type Layout = buffer::BufferLayout;
    type Usage = BufferUsage;
    type Range = Range<u64>;
    const ID: u32 = 0;
}

/// Image resource.
/// Implements `Resource` with associated types required for images.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Image {}
impl Resource for Image {
    type Access = ImageAccess;
    type Layout = ImageLayout;
    type Usage = ImageUsage;
    type Range = SubresourceRange;
    const ID: u32 = 1;
}

/// Resource typed id
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Id<R>(u32, PhantomData<R>);

impl<R> Id<R> {
    /// Create new resource id.
    pub fn new(index: u32) -> Self {
        Id(index, PhantomData)
    }
}

/// Resource id
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Uid(u32, u32);

impl<R> From<Id<R>> for Uid
where
    R: Resource,
{
    fn from(id: Id<R>) -> Uid {
        Uid(R::ID, id.0)
    }
}

/// State of the resource.
#[derive(Clone, Copy, Debug)]
pub struct State<R: Resource> {
    /// Access types for the resource.
    pub access: R::Access,

    /// Current layout of the resource.
    pub layout: R::Layout,

    /// Stages at which resource is accessed.
    pub stages: PipelineStage,
}

impl<R> State<R>
where
    R: Resource,
{
    /// Merge states.
    /// Panic if layouts are incompatible.
    pub fn merge(&self, rhs: Self) -> Self {
        State {
            access: self.access | rhs.access,
            layout: self.layout.merge(rhs.layout).unwrap(),
            stages: self.stages | rhs.stages,
        }
    }

    /// Check if access is exclusive.
    pub fn exclusive(&self) -> bool {
        self.access.is_write()
    }

    /// Check if states are compatible.
    /// This requires layouts to be compatible and non-exclusive access.
    pub fn compatible(&self, rhs: Self) -> bool {
        !self.exclusive() && !rhs.exclusive() && self.layout.merge(rhs.layout).is_some()
    }
}
