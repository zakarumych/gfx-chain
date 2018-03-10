use std::ops::Range;

use hal::command::CommandBuffer;
use hal::memory::Dependencies;
use hal::pso::PipelineStage;
use hal::queue::{QueueFamilyId, Supports, Transfer};

use resource::{Access, Layout, Resource};
use queue::QueueId;

#[derive(Clone, Copy, Debug)]
pub struct Acquire;
#[derive(Clone, Copy, Debug)]
pub struct Release;

pub(super) trait Semantics {
    fn src_dst(this: QueueFamilyId, other: QueueFamilyId) -> (QueueFamilyId, QueueFamilyId);
}

impl Semantics for Acquire {
    fn src_dst(this: QueueFamilyId, other: QueueFamilyId) -> (QueueFamilyId, QueueFamilyId) {
        (other, this)
    }
}

impl Semantics for Release {
    fn src_dst(this: QueueFamilyId, other: QueueFamilyId) -> (QueueFamilyId, QueueFamilyId) {
        (this, other)
    }
}

/// Synchronization required for the link.
#[derive(Clone, Debug)]
pub enum LinkSync<A, L, S, M> {
    /// No transition required.
    None(M),

    /// Pipeline barrier.
    Barrier {
        access: Range<A>,
        layout: Range<L>,
        stages: Range<PipelineStage>,
    },

    /// Signal / Wait for semaphore.
    Semaphore { semaphore: S },

    /// Signal / Wait for semaphore and insert barrier.
    BarrierSemaphore {
        access: Range<A>,
        layout: Range<L>,
        stages: Range<PipelineStage>,
        semaphore: S,
    },

    /// Perform ownership transfer.
    Transfer {
        access: Range<A>,
        layout: Range<L>,
        stages: Range<PipelineStage>,
        semaphore: S,
        other: QueueId,
    },
}

impl<A, L, S, M> LinkSync<A, L, S, M> {
    pub(super) fn is_none(&self) -> bool {
        match *self {
            LinkSync::None(_) => true,
            _ => false,
        }
    }
}

impl<A, L, S> LinkSync<A, L, S, Acquire> {
    /// Report what semaphore should be waited before executing commands of the link.
    pub(super) fn wait(&self) -> Option<&S> {
        match *self {
            LinkSync::None(_) | LinkSync::Barrier { .. } => None,
            LinkSync::Semaphore { ref semaphore }
            | LinkSync::BarrierSemaphore { ref semaphore, .. }
            | LinkSync::Transfer { ref semaphore, .. } => Some(semaphore),
        }
    }
}

impl<A, L, S> LinkSync<A, L, S, Release> {
    /// Report what semaphore should be signaled after executing commands of the link.
    pub(super) fn signal(&self) -> Option<&S> {
        match *self {
            LinkSync::None(_) | LinkSync::Barrier { .. } => None,
            LinkSync::Semaphore { ref semaphore }
            | LinkSync::BarrierSemaphore { ref semaphore, .. }
            | LinkSync::Transfer { ref semaphore, .. } => Some(semaphore),
        }
    }
}

impl<A, L, S, M> LinkSync<A, L, S, M> {
    /// Insert barrier if required before recording commands for the link.
    pub(super) fn barrier<R, C>(
        &self,
        this: QueueId,
        commands: &mut CommandBuffer<R::Backend, C>,
        resources: Option<&[(&R, R::Range)]>,
    ) where
        A: Access,
        L: Layout,
        M: Semantics,
        C: Supports<Transfer>,
        R: Resource<Access = A, Layout = L>,
    {
        let (access, layout, stages, (src, dst)) = match *self {
            LinkSync::None(_) | LinkSync::Semaphore { .. } => {
                return;
            }
            LinkSync::Barrier {
                ref access,
                ref layout,
                ref stages,
            } | LinkSync::BarrierSemaphore {
                ref access,
                ref layout,
                ref stages,
                ..
            } => (access, layout, stages, (this.family(), this.family())),
            LinkSync::Transfer {
                ref access,
                ref layout,
                ref stages,
                other,
                ..
            } => (
                access,
                layout,
                stages,
                M::src_dst(this.family(), other.family()),
            ),
        };
        if src != dst {
            unimplemented!();
        }
        match resources {
            Some(resources) => {
                commands.pipeline_barrier(
                    stages.clone(),
                    Dependencies::empty(),
                    resources.iter().map(|&(resource, ref range)| {
                        resource.barrier(access.clone(), layout.clone(), R::Range::clone(range))
                    }),
                );
            }
            None => {
                assert_eq!(
                    layout.start, layout.end,
                    "Can't use big barrier if layout transition is required"
                );
                assert_eq!(
                    src, dst,
                    "Can't use big barrier if ownership transfer is required"
                );
                commands.pipeline_barrier(
                    stages.clone(),
                    Dependencies::empty(),
                    Some(R::big_barrier(access.clone())),
                );
            }
        }
    }
}

/// Link of the resource chain.
/// Link corresponds to single rendering-pass or similar entity.
/// Rendering pass is expected to use resources associated with chain only in ways specified in the link.
#[derive(Clone, Debug)]
pub struct Link<A, L, S, W> {
    pub(super) queue: QueueId,
    pub(super) stages: PipelineStage,
    pub(super) access: A,
    pub(super) layout: L,
    pub(super) acquire: LinkSync<A, L, W, Acquire>,
    pub(super) release: LinkSync<A, L, S, Release>,
}

impl<A, L, S, W> Link<A, L, S, W>
where
    A: Access,
    L: Layout,
{
    /// Get acquire synchronization info
    pub fn acquire(&self) -> &LinkSync<A, L, W, Acquire> {
        &self.acquire
    }

    /// Get release synchronization info
    pub fn release(&self) -> &LinkSync<A, L, S, Release> {
        &self.release
    }

    /// Get acquire synchronization info
    pub fn take_acquire(&mut self) -> LinkSync<A, L, W, Acquire> {
        use std::mem::replace;
        replace(&mut self.acquire, LinkSync::None(Acquire))
    }

    /// Get release synchronization info
    pub fn take_release(&mut self) -> LinkSync<A, L, S, Release> {
        use std::mem::replace;
        replace(&mut self.release, LinkSync::None(Release))
    }

    /// Get acquire synchronization info
    pub fn set_acquire(&mut self, sync: LinkSync<A, L, W, Acquire>) {
        self.acquire = sync;
    }

    /// Get release synchronization info
    pub fn set_release(&mut self, sync: LinkSync<A, L, S, Release>) {
        self.release = sync;
    }

    /// Get allowed access type for the link.
    pub fn access(&self) -> A {
        self.access
    }

    /// Get resource layout for the link.
    pub fn layout(&self) -> L {
        self.layout
    }

    /// Get stages at which resource is accessed.
    pub fn stages(&self) -> PipelineStage {
        self.stages
    }

    /// Record acquire barrier if required.
    pub fn record_acquire<R, C>(
        &self,
        commands: &mut CommandBuffer<R::Backend, C>,
        resources: Option<&[(&R, R::Range)]>,
    ) where
        R: Resource<Access = A, Layout = L>,
        C: Supports<Transfer>,
    {
        self.acquire.barrier(self.queue, commands, resources);
    }

    /// Record release barrier if required.
    pub fn record_release<R, C>(
        &self,
        commands: &mut CommandBuffer<R::Backend, C>,
        resources: Option<&[(&R, R::Range)]>,
    ) where
        R: Resource<Access = A, Layout = L>,
        C: Supports<Transfer>,
    {
        self.release.barrier(self.queue, commands, resources);
    }

    /// Get waiting token if required.
    pub fn wait(&self) -> Option<&W> {
        self.acquire.wait()
    }

    /// Get signaling token if required.
    pub fn signal(&self) -> Option<&S> {
        self.release.signal()
    }
}
