use hal::pso::PipelineStage;
use std::fmt::Debug;
use std::ops::{BitAnd, BitAndAssign, BitOr, BitOrAssign};

/// Access type combination
pub trait Access:
    Debug + Copy + Eq + BitAnd<Output = Self> + BitAndAssign + BitOr<Output = Self> + BitOrAssign
{
    /// Create empty combinations of access types.
    fn none() -> Self;

    /// Create access instance that combines all possible access types.
    fn all() -> Self;

    /// Check if the access combination contains at least one read access type.
    fn is_read(&self) -> bool;

    /// Check if the access combination contains at least one write access type.
    fn is_write(&self) -> bool;

    /// Get set of supported stages.
    /// This function is valid only for single access type.
    ///
    /// # Panics
    ///
    /// If this access combination has more than one access types this function will panic.
    fn supported_pipeline_stages(&self) -> PipelineStage;
}
