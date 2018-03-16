mod link;

use std::collections::HashMap;
use hal::queue::QueueFamilyId;
use resource::{Buffer, Id, Image, Resource};

pub use self::link::Link;

#[derive(Clone, Debug)]
pub struct Chain<R: Resource> {
    links: Vec<Link<R>>,
}

impl<R> Chain<R>
where
    R: Resource,
{
    /// Get family of last link.
    pub fn last_family(&self) -> Option<QueueFamilyId> {
        self.links.last().map(|link| link.family())
    }

    /// Get links slice
    pub fn links(&self) -> &[Link<R>] {
        &self.links
    }

    /// Get links slice
    pub fn last_link(&self) -> Option<&Link<R>> {
        self.links.last()
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

impl<R> Default for Chain<R>
where
    R: Resource,
{
    fn default() -> Self {
        Chain { links: Vec::new() }
    }
}

pub type BufferChains = HashMap<Id<Buffer>, Chain<Buffer>>;
pub type ImageChains = HashMap<Id<Image>, Chain<Image>>;
