use std::fmt::Debug;

/// Layout specify how resource's content is placed in gpu memory
pub trait Layout: Debug + Copy + Eq + Sized {
    /// Merge this layout and another.
    /// Returns `None` if layouts can't be merged.
    fn merge(self, other: Self) -> Option<Self>;

    /// Get relaxed source layout for transition.
    /// Content may be discarded during transition from the relaxed layout.
    /// Not all layout types have dedicated value for this.
    fn discard_content(self) -> Self {
        self
    }
}
