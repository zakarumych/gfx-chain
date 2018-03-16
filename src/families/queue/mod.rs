mod submit;

use std::ops::{Index, IndexMut};

use hal::queue::QueueFamilyId;

pub use self::submit::{Submit, SubmitId, SubmitInsertLink};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct QueueId {
    family: usize,
    index: usize,
}

impl QueueId {
    pub fn new(family: QueueFamilyId, index: usize) -> Self {
        QueueId {
            family: family.0,
            index,
        }
    }

    pub fn family(&self) -> QueueFamilyId {
        QueueFamilyId(self.family)
    }

    pub fn index(&self) -> usize {
        self.index
    }
}

#[derive(Clone, Debug)]
pub struct Queue {
    id: QueueId,
    submits: Vec<Submit>,
}

impl Queue {
    pub fn new(id: QueueId) -> Self {
        Queue {
            id,
            submits: Vec::default(),
        }
    }

    pub fn len(&self) -> usize {
        self.submits.len()
    }

    pub fn get_submit(&self, sid: SubmitId) -> Option<&Submit> {
        assert_eq!(self.id, sid.queue());
        self.submits.get(sid.index())
    }

    pub fn get_submit_mut(&mut self, sid: SubmitId) -> Option<&mut Submit> {
        assert_eq!(self.id, sid.queue());
        self.submits.get_mut(sid.index())
    }

    pub fn add_submit(&mut self, submit: Submit) -> SubmitId {
        self.submits.push(submit);
        SubmitId::new(self.id, self.submits.len() - 1)
    }
}

impl Index<SubmitId> for Queue {
    type Output = Submit;

    fn index(&self, sid: SubmitId) -> &Submit {
        self.get_submit(sid).unwrap()
    }
}

impl IndexMut<SubmitId> for Queue {
    fn index_mut(&mut self, sid: SubmitId) -> &mut Submit {
        self.get_submit_mut(sid).unwrap()
    }
}
