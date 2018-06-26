use hal::buffer::{Access as BufferAccess, Usage as BufferUsage};
use hal::pso::PipelineStage;

use resource::{Access, Layout, Usage};

impl Access for BufferAccess {
    fn none() -> Self {
        Self::empty()
    }

    fn all() -> Self {
        Self::all()
    }

    fn is_write(&self) -> bool {
        self.contains(Self::TRANSFER_WRITE)
            || self.contains(Self::SHADER_WRITE)
            || self.contains(Self::HOST_WRITE)
            || self.contains(Self::MEMORY_WRITE)
    }

    fn is_read(&self) -> bool {
        self.contains(Self::TRANSFER_READ)
            || self.contains(Self::SHADER_READ)
            || self.contains(Self::HOST_READ)
            || self.contains(Self::MEMORY_READ)
            || self.contains(Self::INDEX_BUFFER_READ)
            || self.contains(Self::VERTEX_BUFFER_READ)
            || self.contains(Self::INDIRECT_COMMAND_READ)
            || self.contains(Self::CONSTANT_BUFFER_READ)
    }

    fn supported_pipeline_stages(&self) -> PipelineStage {
        type PS = PipelineStage;

        match *self {
            Self::TRANSFER_READ | Self::TRANSFER_WRITE => PS::TRANSFER,
            Self::INDEX_BUFFER_READ | Self::VERTEX_BUFFER_READ => PS::VERTEX_INPUT,
            Self::INDIRECT_COMMAND_READ => PS::DRAW_INDIRECT,
            Self::CONSTANT_BUFFER_READ | Self::SHADER_READ | Self::SHADER_WRITE => {
                PS::VERTEX_SHADER | PS::GEOMETRY_SHADER | PS::FRAGMENT_SHADER | PS::COMPUTE_SHADER
            }
            Self::HOST_READ | Self::HOST_WRITE => PS::HOST,
            Self::MEMORY_READ | Self::MEMORY_WRITE => PS::empty(),
            _ => panic!("Only one bit must be set"),
        }
    }
}

/// Buffers can be placed in memory only linearly
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BufferLayout;

impl Layout for BufferLayout {
    fn merge(self, _other: BufferLayout) -> Option<BufferLayout> {
        Some(BufferLayout)
    }

    fn discard_content() -> Self {
        BufferLayout
    }
}

impl Usage for BufferUsage {
    fn none() -> Self {
        BufferUsage::empty()
    }
    fn all() -> Self {
        BufferUsage::all()
    }
}
