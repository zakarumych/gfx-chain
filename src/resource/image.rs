use hal::image::{Access as ImageAccess, Layout as ImageLayout, Usage as ImageUsage};
use hal::pso::PipelineStage;

use resource::{Access, Layout, Usage};

impl Access for ImageAccess {
    fn none() -> Self {
        Self::empty()
    }
    fn all() -> Self {
        Self::all()
    }
    fn is_write(&self) -> bool {
        self.contains(Self::COLOR_ATTACHMENT_WRITE)
            || self.contains(Self::DEPTH_STENCIL_ATTACHMENT_WRITE)
            || self.contains(Self::TRANSFER_WRITE) || self.contains(Self::SHADER_WRITE)
            || self.contains(Self::HOST_WRITE) || self.contains(Self::MEMORY_WRITE)
    }

    fn is_read(&self) -> bool {
        self.contains(Self::COLOR_ATTACHMENT_READ)
            || self.contains(Self::DEPTH_STENCIL_ATTACHMENT_READ)
            || self.contains(Self::TRANSFER_READ) || self.contains(Self::SHADER_READ)
            || self.contains(Self::HOST_READ) || self.contains(Self::MEMORY_READ)
            || self.contains(Self::INPUT_ATTACHMENT_READ)
    }

    fn supported_pipeline_stages(&self) -> PipelineStage {
        type PS = PipelineStage;

        match *self {
            Self::COLOR_ATTACHMENT_READ | Self::COLOR_ATTACHMENT_WRITE => {
                PS::COLOR_ATTACHMENT_OUTPUT
            }
            Self::TRANSFER_READ | Self::TRANSFER_WRITE => PS::TRANSFER,
            Self::SHADER_READ | Self::SHADER_WRITE => {
                PS::VERTEX_SHADER | PS::GEOMETRY_SHADER | PS::FRAGMENT_SHADER | PS::COMPUTE_SHADER
            }
            Self::DEPTH_STENCIL_ATTACHMENT_READ | Self::DEPTH_STENCIL_ATTACHMENT_WRITE => {
                PS::EARLY_FRAGMENT_TESTS | PS::LATE_FRAGMENT_TESTS
            }
            Self::HOST_READ | Self::HOST_WRITE => PS::HOST,
            Self::MEMORY_READ | Self::MEMORY_WRITE => PS::empty(),
            Self::INPUT_ATTACHMENT_READ => PS::FRAGMENT_SHADER,
            _ => panic!("Only one bit must be set"),
        }
    }
}

impl Layout for ImageLayout {
    fn merge(self, other: ImageLayout) -> Option<ImageLayout> {
        match (self, other) {
            (x, y) if x == y => Some(x),
            (ImageLayout::Present, _) | (_, ImageLayout::Present) => None,
            (ImageLayout::ShaderReadOnlyOptimal, ImageLayout::DepthStencilReadOnlyOptimal)
            | (ImageLayout::DepthStencilReadOnlyOptimal, ImageLayout::ShaderReadOnlyOptimal) => {
                Some(ImageLayout::DepthStencilReadOnlyOptimal)
            }
            (_, _) => Some(ImageLayout::General),
        }
    }

    fn discard_content() -> Self {
        ImageLayout::Undefined
    }
}

impl Usage for ImageUsage {
    fn none() -> Self {
        ImageUsage::empty()
    }
    fn all() -> Self {
        ImageUsage::all()
    }
}
