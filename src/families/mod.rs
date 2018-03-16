mod queue;

use std::collections::hash_map::{HashMap, Iter as HashMapIter};
use std::ops::{Index, IndexMut};

use hal::queue::QueueFamilyId;

pub use self::queue::{Queue, QueueId, Submit, SubmitId, SubmitInsertLink};

#[derive(Clone, Debug)]
pub struct Family {
    id: QueueFamilyId,
    queues: Vec<Queue>,
}

impl Family {
    pub fn new(id: QueueFamilyId) -> Self {
        Family {
            id,
            queues: Vec::default(),
        }
    }

    pub fn get_queue(&self, qid: QueueId) -> Option<&Queue> {
        assert_eq!(self.id, qid.family());
        self.queues.get(qid.index())
    }

    pub fn get_queue_mut(&mut self, qid: QueueId) -> Option<&mut Queue> {
        assert_eq!(self.id, qid.family());
        self.queues.get_mut(qid.index())
    }

    pub fn ensure_queue(&mut self, qid: QueueId) -> &mut Queue {
        assert_eq!(self.id, qid.family());
        let len = self.queues.len();
        self.queues
            .extend((len..qid.index() + 1).map(|_| Queue::new(qid)));
        &mut self.queues[qid.index()]
    }

    pub fn get_submit(&self, sid: SubmitId) -> Option<&Submit> {
        assert_eq!(self.id, sid.family());
        self.get_queue(sid.queue())
            .and_then(|queue| queue.get_submit(sid))
    }

    pub fn get_submit_mut(&mut self, sid: SubmitId) -> Option<&mut Submit> {
        assert_eq!(self.id, sid.family());
        self.get_queue_mut(sid.queue())
            .and_then(|queue| queue.get_submit_mut(sid))
    }
}

impl Index<QueueId> for Family {
    type Output = Queue;

    fn index(&self, qid: QueueId) -> &Queue {
        self.get_queue(qid).unwrap()
    }
}

impl IndexMut<QueueId> for Family {
    fn index_mut(&mut self, qid: QueueId) -> &mut Queue {
        self.get_queue_mut(qid).unwrap()
    }
}

impl Index<SubmitId> for Family {
    type Output = Submit;

    fn index(&self, sid: SubmitId) -> &Submit {
        self.get_submit(sid).unwrap()
    }
}

impl IndexMut<SubmitId> for Family {
    fn index_mut(&mut self, sid: SubmitId) -> &mut Submit {
        self.get_submit_mut(sid).unwrap()
    }
}

pub struct Families {
    map: HashMap<QueueFamilyId, Family>,
}

impl Families {
    pub fn iter(&self) -> HashMapIter<QueueFamilyId, Family> {
        self.map.iter()
    }

    pub fn get_family(&self, fid: QueueFamilyId) -> Option<&Family> {
        self.map.get(&fid)
    }

    pub fn get_family_mut(&mut self, fid: QueueFamilyId) -> Option<&mut Family> {
        self.map.get_mut(&fid)
    }

    pub fn ensure_family(&mut self, fid: QueueFamilyId) -> &mut Family {
        self.map.entry(fid).or_insert_with(|| Family::new(fid))
    }

    pub fn get_queue(&self, qid: QueueId) -> Option<&Queue> {
        self.get_family(qid.family())
            .and_then(|family| family.get_queue(qid))
    }

    pub fn get_queue_mut(&mut self, qid: QueueId) -> Option<&mut Queue> {
        self.get_family_mut(qid.family())
            .and_then(|family| family.get_queue_mut(qid))
    }

    pub fn ensure_queue(&mut self, qid: QueueId) -> &mut Queue {
        self.ensure_family(qid.family()).ensure_queue(qid)
    }

    pub fn get_submit(&self, sid: SubmitId) -> Option<&Submit> {
        self.get_queue(sid.queue())
            .and_then(|queue| queue.get_submit(sid))
    }

    pub fn get_submit_mut(&mut self, sid: SubmitId) -> Option<&mut Submit> {
        self.get_queue_mut(sid.queue())
            .and_then(|queue| queue.get_submit_mut(sid))
    }
}

impl Default for Families {
    fn default() -> Self {
        Families {
            map: HashMap::default(),
        }
    }
}

impl Index<QueueFamilyId> for Families {
    type Output = Family;

    fn index(&self, fid: QueueFamilyId) -> &Family {
        self.get_family(fid).unwrap()
    }
}

impl IndexMut<QueueFamilyId> for Families {
    fn index_mut(&mut self, fid: QueueFamilyId) -> &mut Family {
        self.get_family_mut(fid).unwrap()
    }
}

impl Index<QueueId> for Families {
    type Output = Queue;

    fn index(&self, qid: QueueId) -> &Queue {
        self.get_queue(qid).unwrap()
    }
}

impl IndexMut<QueueId> for Families {
    fn index_mut(&mut self, qid: QueueId) -> &mut Queue {
        self.get_queue_mut(qid).unwrap()
    }
}

impl Index<SubmitId> for Families {
    type Output = Submit;

    fn index(&self, sid: SubmitId) -> &Submit {
        self.get_submit(sid).unwrap()
    }
}

impl IndexMut<SubmitId> for Families {
    fn index_mut(&mut self, sid: SubmitId) -> &mut Submit {
        self.get_submit_mut(sid).unwrap()
    }
}
