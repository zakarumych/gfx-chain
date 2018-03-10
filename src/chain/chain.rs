use std::borrow::Borrow;
use std::marker::PhantomData;

use hal::pso::PipelineStage;

use chain::pass::PassLinks;
use chain::link::{Acquire, Link, LinkSync, Release};
use resource::{Access, Layout, Resource, Usage};

/// Unique identifier for resource dependency chain.
/// Multiple resource can be associated with single chain
/// if all passes uses them the same way.
/// Chain id uses marker type so that ids for buffers and images are different.
#[derive(Derivative)]
#[derivative(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct ChainId<T: ?Sized>(usize, PhantomData<T>);
impl<T> ChainId<T> {
    /// Make new chain id.
    pub fn new(index: usize) -> Self {
        ChainId(index, PhantomData)
    }

    /// Get index value.
    pub fn index(&self) -> usize {
        self.0
    }
}

/// Resource chain for set of resources.
/// All resources in set are expected to be used only in ways specified by this chain.
/// Each link of the chain corresponds to some render-pass or similar entity.
#[derive(Clone, Debug)]
pub struct Chain<A, L, U, S, W = S> {
    usage: U,
    links: Vec<Option<Link<A, L, S, W>>>,
}

impl<A, L, U, S, W> Chain<A, L, U, S, W>
where
    A: Access,
    L: Layout,
    U: Usage,
{
    /// Build chain from links defined by passes.
    pub fn build<P, R, F>(id: ChainId<R>, mut usage: U, passes: P, mut new_semaphore: F) -> Self
    where
        P: IntoIterator,
        P::Item: Borrow<PassLinks<R>>,
        R: Resource<Access = A, Layout = L, Usage = U>,
        F: FnMut() -> (S, W),
    {
        let mut links: Vec<Option<Link<A, L, S, W>>> = Vec::new();

        // Walk over passes
        for pass in passes {
            let pass = pass.borrow();
            // Collect links from passes.
            links.push(pass.links.iter().find(|link| link.id == id).map(|link| {
                usage = usage | link.usage;
                Link {
                    queue: pass.queue,
                    access: link.access,
                    layout: link.layout,
                    stages: link.stages,
                    acquire: LinkSync::None(Acquire),
                    release: LinkSync::None(Release),
                }
            }));
        }

        let count = links.len();

        // Walk over all links twice and merge states of compatible sub-chains
        for index in 0..(count * 2) {
            let index = index % count;
            let (before, link_after) = links.split_at_mut(index);
            let (link, after) = link_after.split_first_mut().unwrap();

            // Skip non-existing
            let link = if let Some(link) = link.as_mut() {
                link
            } else {
                continue;
            };

            // Get next existing link
            if let Some(next) = after
                .iter_mut()
                .chain(before.iter_mut())
                .filter_map(Option::as_mut)
                .next()
            {
                let compatible = !link.access.is_write() && !next.access.is_write();
                let access = link.access | next.access;
                let stages = link.stages | next.stages;
                match link.layout.merge(next.layout) {
                    Some(layout) if compatible && link.queue == next.queue => {
                        link.layout = layout;
                        next.layout = layout;
                        link.access = access;
                        next.access = access;
                        link.stages = stages;
                        next.stages = stages;
                    }
                    _ => {}
                }
            } else {
                // No other links
                break;
            }
        }

        for index in 0..count {
            let (before, link_after) = links.split_at_mut(index);
            let (link, after) = link_after.split_first_mut().unwrap();

            // Skip non-existing
            let link = if let Some(link) = link.as_mut() {
                link
            } else {
                continue;
            };

            if let Some(next) = after
                .iter_mut()
                .chain(before.iter_mut())
                .filter_map(Option::as_mut)
                .next()
            {
                debug_assert!(link.release.is_none());
                debug_assert!(next.acquire.is_none());

                let compatible = !link.access.is_write() && !next.access.is_write();

                let src_layout = if next.access.is_read() {
                    link.layout
                } else {
                    link.layout.discard_content()
                };
                let dst_layout = next.layout;

                let src_access = link.access;
                let dst_access = next.access;

                let src_stages = link.stages;
                let dst_stages = link.stages;

                match link.layout.merge(next.layout) {
                    Some(_) if compatible && link.queue == next.queue => {
                        // Verify that they are merged properly
                    }
                    _ if link.queue == next.queue => {
                        // Incompatible states on same queue. Insert barrier.
                        link.release = LinkSync::Barrier {
                            access: src_access..dst_access,
                            layout: src_layout..dst_layout,
                            stages: src_stages..dst_stages,
                        };
                    }
                    _ if link.queue.family() == next.queue.family() || !next.access.is_read() => {
                        // Same family.
                        // Or different family but content is discarded.
                        // Ownership transfer should be skipped. See note at
                        // https://www.khronos.org/registry/vulkan/specs/1.0/html/vkspec.html#synchronization-queue-transfers
                        let (signal, wait) = new_semaphore();

                        if src_layout == dst_layout {
                            // Semaphores creates access scope for `A::all() .. A::none() - A::none() .. A::all()`
                            // Since no layout transition required barrier can be skipped.
                            link.release = LinkSync::Semaphore { semaphore: signal };
                            next.acquire = LinkSync::Semaphore { semaphore: wait };
                        } else {
                            // Perform layout transition after waiting for semaphore.
                            link.release = LinkSync::Semaphore { semaphore: signal };
                            next.acquire = LinkSync::BarrierSemaphore {
                                semaphore: wait,
                                access: A::none() .. dst_access,
                                layout: src_layout .. dst_layout,
                                stages: PipelineStage::empty() .. dst_stages,
                            };
                        }
                    }
                    _ => {
                        let (signal, wait) = new_semaphore();

                        // Different families. Content must be preserved.
                        // Perform ownership transfer according to
                        // https://www.khronos.org/registry/vulkan/specs/1.0/html/vkspec.html#synchronization-queue-transfers
                        link.release = LinkSync::Transfer {
                            semaphore: signal,
                            access: src_access..A::none(),
                            layout: src_layout..dst_layout,
                            stages: src_stages..PipelineStage::empty(),
                            other: next.queue,
                        };
                        next.acquire = LinkSync::Transfer {
                            semaphore: wait,
                            access: A::none()..A::all(),
                            layout: src_layout..dst_layout,
                            stages: PipelineStage::empty()..dst_stages,
                            other: link.queue,
                        };
                    }
                }
            } else {
                // No other links
                break;
            }
        }

        if links.iter().all(Option::is_none) {
            warn!("Empty chain {:?}", id);
        }

        Chain { links, usage }
    }
}

impl<A, L, U, S, W> Chain<A, L, U, S, W> {
    /// Get number of links in chain.
    pub fn len(&self) -> usize {
        self.links.len()
    }

    /// Get reference to links.
    pub fn links(&self) -> &[Option<Link<A, L, S, W>>] {
        &self.links
    }

    /// Get reference to link by index.
    ///
    /// # Panics
    ///
    /// This function will panic if no link exists at this index.
    ///
    pub fn link(&self, index: usize) -> &Link<A, L, S, W> {
        self.links[index].as_ref().unwrap()
    }

    /// Get mutable reference to link by index.
    ///
    /// # Panics
    ///
    /// This function will panic if no link exists at this index.
    ///
    pub fn link_mut(&mut self, index: usize) -> &mut Link<A, L, S, W> {
        self.links[index].as_mut().unwrap()
    }

    /// Get reference to first link before specified index.
    pub fn prev(&self, index: usize) -> Option<usize> {
        let (before, _) = self.links.split_at(index);
        before.iter().enumerate()
            .rev()
            .filter(|&(_, l)| l.is_some())
            .next().map(|(i, _)| i)
    }

    /// Get reference to first link before specified index.
    /// Links at the end treated as before index 0.
    pub fn prev_wrapping(&self, index: usize) -> Option<usize> {
        let (before, link_after) = self.links.split_at(index);
        let after = link_after.split_first().map(|(_, after)| after).unwrap_or(&[]);
        before.iter().enumerate()
            .rev()
            .chain(after.iter().enumerate().rev().map(|(i, l)| (i + index + 1, l)))
            .filter(|&(_, l)| l.is_some())
            .next().map(|(i, _)| i)
    }

    /// Get reference to first link not before specified index.
    pub fn next(&self, index: usize) -> Option<usize> {
        let (_, link_after) = self.links.split_at(index);
        let after = link_after.split_first().map(|(_, after)| after).unwrap_or(&[]);
        after
            .iter()
            .enumerate().map(|(i, l)| (i + index + 1, l))
            .filter(|&(_, l)| l.is_some())
            .next().map(|(i, _)| i)
    }

    /// Get reference to first link _not_ _before_ specified index.
    /// Links at the beginning treated as after last index.
    pub fn next_wrapping(&self, index: usize) -> Option<usize> {
        let (before, link_after) = self.links.split_at(index);
        let after = link_after.split_first().map(|(_, after)| after).unwrap_or(&[]);
        after
            .iter()
            .enumerate().map(|(i, l)| (i + index + 1, l))
            .chain(before.iter().enumerate())
            .filter(|&(_, l)| l.is_some())
            .next().map(|(i, _)| i)
    }

    /// Get reference to first link.
    ///
    /// # Panics
    ///
    /// This function will panic if no link exists.
    pub fn first(&self) -> usize {
        self.links.iter().enumerate().filter(|&(_, l)| l.is_some()).next().unwrap().0
    }

    /// Get reference to last link.
    ///
    /// # Panics
    ///
    /// This function will panic if no link exists.
    pub fn last(&self) -> usize {
        self.links
            .iter()
            .enumerate()
            .rev()
            .filter(|&(_, l)| l.is_some())
            .next()
            .unwrap().0
    }

    /// Get combination of all usage types for the resource
    pub fn usage(&self) -> &U {
        &self.usage
    }
}
