use std::ops::{Index, IndexMut};

use super::chain::{Chain, ChainId};
use super::pass::PassLinks;
use resource::{Resource, Usage};

/// Type alias for `Chain` bound to specific resource type.
pub type ResourceChain<R: Resource, S, W> = Chain<R::Access, R::Layout, R::Usage, S, W>;

/// Collection of all dependency chains.
#[derive(Clone, Debug, Default)]
pub struct ResourceChains<R: Resource, S, W> {
    chains: Vec<ResourceChain<R, S, W>>,
}

impl<R, S, W> ResourceChains<R, S, W>
where
    R: Resource,
{
    /// Create new instance of `ResourceChains`.
    /// 
    /// # Parameters
    /// 
    /// `passes`        - instances of `PassLinks` collected from all passes.
    /// `new_semaphore` - function to allocate new semaphore as a pair of tokens. One for signalling. Another for waiting.
    /// 
    pub fn new<F>(
        passes: &[PassLinks<R>],
        mut new_semaphore: F,
    ) -> ResourceChains<R, S, W>
    where
        F: FnMut() -> (S, W),
    {
        let total_count = passes.iter().flat_map(|pass| pass.links.iter().map(|link| link.id.index())).max().unwrap_or(0);

        ResourceChains {
            chains: (0..total_count)
                .map(|i| Chain::build(ChainId::new(i), R::Usage::none(), passes, &mut new_semaphore))
                .collect(),
        }
    }

    /// Get reference to chain by id.
    pub fn chain(&self, id: ChainId<R>) -> &ResourceChain<R, S, W> {
        &self.chains[id.index()]
    }

    /// Get mutable reference to chain by id.
    pub fn chain_mut(&mut self, id: ChainId<R>) -> &mut ResourceChain<R, S, W> {
        &mut self.chains[id.index()]
    }
}

impl<R, S, W> Index<ChainId<R>> for ResourceChains<R, S, W>
where
    R: Resource,
{
    type Output = ResourceChain<R, S, W>;
    fn index(&self, index: ChainId<R>) -> &ResourceChain<R, S, W> {
        self.chain(index)
    }
}

impl<R, S, W> IndexMut<ChainId<R>> for ResourceChains<R, S, W>
where
    R: Resource,
{
    fn index_mut(&mut self, index: ChainId<R>) -> &mut ResourceChain<R, S, W> {
        self.chain_mut(index)
    }
}
