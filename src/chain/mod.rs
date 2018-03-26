//!
//!
//! This module defines types to reason about what resources referenced in what submissions.
//! How commands from those submissions access resources.
//! This information allows to derive synchronization required.
//!

mod link;

use std::collections::HashMap;
use resource::{Buffer, Id, Image, Resource};

pub use self::link::Link;

/// This type corresponds to resource category.
/// All resources from the same category must be accessed as permitted by links of the chain.
#[derive(Clone, Debug)]
pub struct Chain<R: Resource> {
    links: Vec<Link<R>>,
}

impl<R> Chain<R>
where
    R: Resource,
{
    /// Get links slice
    pub fn links(&self) -> &[Link<R>] {
        &self.links
    }

    /// Create new empty `Chain`
    pub fn new() -> Self {
        Chain {
            links: Vec::new()
        }
    }

    /// Get links slice
    pub fn last_link_mut(&mut self) -> Option<&mut Link<R>> {
        self.links.last_mut()
    }

    /// Add new link to the chain.
    pub fn add_link(&mut self, link: Link<R>) -> &mut Link<R> {
        self.links.push(link);
        self.links.last_mut().unwrap()
    }
}

/// Type alias for map of chains by id for buffers.
pub type BufferChains = HashMap<Id<Buffer>, Chain<Buffer>>;

/// Type alias for map of chains by id for images.
pub type ImageChains = HashMap<Id<Image>, Chain<Image>>;
