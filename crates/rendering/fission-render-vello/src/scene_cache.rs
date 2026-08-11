use std::collections::{HashMap, VecDeque};

use vello::Scene;

pub struct RetainedSceneCache {
    entries: HashMap<u64, Scene>,
    order: VecDeque<u64>,
    max_entries: usize,
}

impl Default for RetainedSceneCache {
    fn default() -> Self {
        Self::new(256)
    }
}

impl RetainedSceneCache {
    pub fn new(max_entries: usize) -> Self {
        Self {
            entries: HashMap::new(),
            order: VecDeque::new(),
            max_entries: max_entries.max(1),
        }
    }

    pub fn clear(&mut self) {
        self.entries.clear();
        self.order.clear();
    }

    pub fn contains(&self, key: u64) -> bool {
        self.entries.contains_key(&key)
    }

    pub fn get(&self, key: u64) -> Option<&Scene> {
        self.entries.get(&key)
    }

    pub fn insert(&mut self, key: u64, scene: Scene) {
        if self.entries.contains_key(&key) {
            self.entries.insert(key, scene);
            return;
        }
        while self.entries.len() >= self.max_entries {
            if let Some(oldest) = self.order.pop_front() {
                self.entries.remove(&oldest);
            } else {
                break;
            }
        }
        self.order.push_back(key);
        self.entries.insert(key, scene);
    }

    pub fn get_or_insert_with<F>(&mut self, key: u64, build: F) -> anyhow::Result<&Scene>
    where
        F: FnOnce(&mut RetainedSceneCache) -> anyhow::Result<Scene>,
    {
        if !self.entries.contains_key(&key) {
            let scene = build(self)?;
            self.insert(key, scene);
        }
        Ok(self
            .entries
            .get(&key)
            .expect("scene cache entry missing after insertion"))
    }
}
