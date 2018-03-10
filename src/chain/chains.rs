use std::ops::{Index, IndexMut};

use super::chain::{Chain, ChainId};
use super::pass::PassLinks;
use resource::{Access, Layout, Resource, Usage};

/// Collection of all dependency chains.
#[derive(Clone, Debug, Default)]
pub struct ChainSet<A, L, U, S, W = S> {
    chains: Vec<Chain<A, L, U, S, W>>,
}

impl<A, L, U, S, W> ChainSet<A, L, U, S, W>
where
    A: Access,
    L: Layout,
    U: Usage,
{
    /// Create new instance of `ResourceChainSet`.
    /// 
    /// # Parameters
    /// 
    /// `passes`        - instances of `PassLinks` collected from all passes.
    /// `new_semaphore` - function to allocate new semaphore as a pair of tokens. One for signalling. Another for waiting.
    /// 
    pub fn new<R, F>(
        passes: &[PassLinks<R>],
        mut new_semaphore: F,
    ) -> Self
    where
        R: Resource<Access = A, Layout = L, Usage = U>,
        F: FnMut() -> (S, W),
    {
        let total_count = passes.iter().flat_map(|pass| pass.links.iter().map(|link| link.id.index())).max().unwrap_or(0);

        ChainSet {
            chains: (0..total_count)
                .map(|i| Chain::build(ChainId::new(i), R::Usage::none(), passes, &mut new_semaphore))
                .collect(),
        }
    }

    /// Get reference to chain by id.
    pub fn chain<R>(&self, id: ChainId<R>) -> &Chain<A, L, U, S, W>
    where
        R: Resource<Access = A, Layout = L, Usage = U>,
    {
        &self.chains[id.index()]
    }

    /// Get mutable reference to chain by id.
    pub fn chain_mut<R>(&mut self, id: ChainId<R>) -> &mut Chain<A, L, U, S, W>
    where
        R: Resource<Access = A, Layout = L, Usage = U>,
    {
        &mut self.chains[id.index()]
    }
}

impl<R, A, L, U, S, W> Index<ChainId<R>> for ChainSet<A, L, U, S, W>
where
    A: Access,
    L: Layout,
    U: Usage,
    R: Resource<Access = A, Layout = L, Usage = U>,
{
    type Output = Chain<A, L, U, S, W>;
    fn index(&self, index: ChainId<R>) -> &Chain<A, L, U, S, W> {
        self.chain(index)
    }
}

impl<R, A, L, U, S, W> IndexMut<ChainId<R>> for ChainSet<A, L, U, S, W>
where
    A: Access,
    L: Layout,
    U: Usage,
    R: Resource<Access = A, Layout = L, Usage = U>,
{
    fn index_mut(&mut self, index: ChainId<R>) -> &mut Chain<A, L, U, S, W> {
        self.chain_mut(index)
    }
}

