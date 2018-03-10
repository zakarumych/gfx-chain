use std::ops::Range;

use hal::command::{ClearValue};
use hal::format::Format;
use hal::pass::{Attachment, AttachmentOps, AttachmentLoadOp, AttachmentStoreOp, AttachmentLayout, SubpassDependency, SubpassRef, SubpassDesc};

use resource::Access;
use chain::{LinkSync, Acquire, Release};
use image::{ImageChain, ImageChainSet};


/// Methods specific for image chains.
impl<S, W> ImageChain<S, W> {
    /// Get attachment description for single render-pass.
    /// Render-pass consist of one or more sub-passes.
    /// Each sub-pass associated with link index.
    /// 
    /// # Parameters
    /// 
    /// `format`    - format of the image.
    /// `clear`     - specify if the attachment should be cleared when link discards content.
    /// `indices`   - open-ended range of indices of sub-passes in chain.
    /// 
    /// # Panics
    /// 
    /// If `indices.start` is bigger than `indices.end` function will panic.
    /// 
    pub fn color_attachment(&self, format: Option<Format>, clear: Option<ClearValue>, indices: Range<usize>) -> Option<Attachment> {
        assert!(indices.start <= indices.end);

        // Get first pass that uses the attachment.
        if let Some(first_index) = self.next(indices.start) {
            if first_index >= indices.end {
                return None;
            }
            let first = self.link(first_index);

            // Calculate attachment layout transition.
            let layout_start = match *first.acquire() {
                LinkSync::None(Acquire) | LinkSync::Semaphore { .. } => {
                    first.layout()
                }
                LinkSync::Barrier { ref layout, .. } | LinkSync::BarrierSemaphore { ref layout, .. } | LinkSync::Transfer { ref layout, .. } => {
                    layout.start
                }
            };

            let last = self.link(self.prev(indices.end).expect("At least one link must be found"));

            let layout_end = match *last.release() {
                LinkSync::None(Release) | LinkSync::Semaphore { .. } => {
                    first.layout()
                }
                LinkSync::Barrier { ref layout, .. } | LinkSync::BarrierSemaphore { ref layout, .. } | LinkSync::Transfer { ref layout, .. } => {
                    layout.end
                }
            };

            // Calculate operations
            let load = match (first.access().is_read(), clear) {
                (true, _) => AttachmentLoadOp::Load,
                (false, Some(_)) => AttachmentLoadOp::Clear,
                (false, None) => AttachmentLoadOp::DontCare,
            };

            let next = self.link(self.next_wrapping(first_index + 1).expect("There is at least one link"));

            let store = if next.access().is_read() {
                AttachmentStoreOp::Store
            } else {
                AttachmentStoreOp::DontCare
            };

            Some(Attachment {
                format,
                ops: AttachmentOps {
                    load,
                    store,
                },
                stencil_ops: AttachmentOps::DONT_CARE,
                layouts: layout_start .. layout_end,
            })
        } else {
            None
        }
    }

    /// Get sub-pass layouts for attachments
    pub fn subpass_layout(&self, index: usize) -> AttachmentLayout {
        self.link(index).layout()
    }

    /// Get sub-pass dependencies for render-pass.
    /// This function will remove barriers translated to sub-pass dependencies.
    pub fn subpass_dependencies(&mut self, indices: Range<usize>) -> HashMap<Range<SubpassRef>, (Range<ImageAccess>, Range<PipelineStage>)> {
        let mut dependencies = HashMap::new();

        let insert_dependency = |from, to, access, stages| {
            debug_assert!(from < indices.end);
            debug_assert!(to < indices.end);
            let dep = dependencies.entry(SubpassRef::Pass(from - indices.start) .. SubpassRef::Pass(to - indices.start))
                .or_insert((A::none() .. A::none(), PipelineStage::empty() .. PipelineStage::empty()));
            dep.0.start |= access.start;
            dep.0.end |= access.end;
            dep.1.start |= stages.start;
            dep.1.end |= stages.end;
        };

        assert!(indices.start <= indices.end);
        let mut next_index = indices.start;

        while let Some(index) = self.next(next_index) {
            if index >= indices.end {
                return dependencies;
            }
            let link = self.link(first_index);
            if self.prev(index).map_or(false, |prev| prev >= indices.start) {
                match link.take_acquire() {
                    LinkSync::Barrier {
                        access,
                        stages,
                    } => insert_dependency(self.prev(index).unwrap(), index, access, stages),
                    LinkSync::BarrierSemaphore {
                        access,
                        stages,
                        semaphore,
                    } => {
                        insert_dependency(self.prev(index).unwrap(), index, access, stages);
                        link.set_acquire(LinkSync::Semaphore { semaphore });
                    }
                    LinkSync::Transfer { .. } => {
                        panic!("Ownership transfer cannot be required between sub-passes");
                    }
                    sync => link.set_acquire(sync),
                }
            }
            if let Some(next_index) = self.next(first_index + 1) {
                let next = self.link(next_index);
                match next.take_release() {
                    LinkSync::Barrier {
                        access,
                        stages,
                    } => insert_dependency(index, next_index, access, stages),
                    LinkSync::BarrierSemaphore {
                        access,
                        stages,
                        semaphore,
                    } => {
                        insert_dependency(index, next_index, access, stages);
                        next.set_release(LinkSync::Semaphore { semaphore })
                    }
                    LinkSync::Transfer { .. } => {
                        panic!("Ownership transfer cannot be required between sub-passes");
                    }
                    sync => link.set_acquire(sync),
                }
            }
        
        } else {
            dependencies
        }
    }
}

impl<S, W> ImageChainSet<S, W> {
    /// Create render-pass from sub-passes at specified indices.
    pub fn create_render_pass<B: Backend>(&mut self, device: &B::Device, attachments: &[ImageChainId], subpasses: &[SubpassDesc], first: usize) -> B::RenderPass {
        unimplemented!()        
    }
}