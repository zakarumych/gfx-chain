mod access;
mod layout;
mod usage;

use std::ops::Range;
use hal::Backend;
use hal::memory::Barrier;

pub use self::access::Access;
pub use self::layout::Layout;
pub use self::usage::Usage;

/// Defines resource type.
/// Should be implemented for buffers and images.
pub trait Resource {
    /// gfx-hal backend of the resource.
    type Backend: Backend;

    /// Access type of the resource.
    type Access: Access;

    /// Layout type of the resource.
    type Layout: Layout;

    /// Usage type of the resource.
    type Usage: Usage;

    /// Sub-resource range.
    type Range: Clone;

    /// Create big barrier that will affect all resources.
    fn big_barrier<'a>(access: Range<Self::Access>) -> Barrier<'a, Self::Backend>;

    /// Create barrier that will affect range of specified resource.
    fn barrier<'a>(
        &self,
        access: Range<Self::Access>,
        layout: Range<Self::Layout>,
        range: Self::Range,
    ) -> Barrier<'a, Self::Backend>;
}
