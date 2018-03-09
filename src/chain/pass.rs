use hal::pso::PipelineStage;

use chain::chain::ChainId;
use resource::Resource;
use queue::QueueId;

/// Piece of `Chain` associated with single pass.
/// Specifies usage, access and layout of the resource.
/// Links can be combined to form resource chains.
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct PassLink<R: Resource> {
    /// Id of the chain.
    pub id: ChainId<R>,

    /// Stages at which pass uses resource.
    pub stages: PipelineStage,

    /// Combination of access types to the resource required by pass.
    pub access: R::Access,

    /// Layout of the resource required by pass.
    pub layout: R::Layout,

    /// Combination of usage types of the resource required by pass.
    pub usage: R::Usage,
}

/// All links defines by single pass.
#[derive(Clone, Debug)]
pub struct PassLinks<R: Resource> {
    /// Queue identifier on which commands recorded by the pass will be executed.
    pub queue: QueueId,

    /// List of links this pass uses.
    pub links: Vec<PassLink<R>>,
}
