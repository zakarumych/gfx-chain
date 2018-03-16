use std::fmt::Debug;
use std::ops::{BitOr, BitOrAssign};

/// Usage type combination
pub trait Usage: Debug + Copy + BitOr<Output = Self> + BitOrAssign + Eq {
    /// Create empty combinations of usage types.
    fn none() -> Self;

    /// Create usage instance that combines all possible usage types
    fn all() -> Self;
}
