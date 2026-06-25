use std::collections::HashMap;
use std::time::{Duration, Instant};

const CACHE_TTL: Duration = Duration::from_secs(300);

struct Timed<T> {
    value: T,
    fetched_at: Instant,
}

impl<T: Clone> Timed<T> {
    fn new(value: T) -> Self {
        Self {
            value,
            fetched_at: Instant::now(),
        }
    }

    fn is_valid(&self) -> bool {
        self.fetched_at.elapsed() < CACHE_TTL
    }
}

#[derive(Default)]
pub(crate) struct ApiCache {
    unavailable_mods: HashMap<u64, Timed<()>>,
    dependencies: HashMap<u64, Timed<Vec<u64>>>,
}

impl ApiCache {
    pub(crate) fn clear(&mut self) {
        self.unavailable_mods.clear();
        self.dependencies.clear();
    }

    pub(crate) fn is_mod_unavailable(&self, mod_id: u64) -> bool {
        self.unavailable_mods
            .get(&mod_id)
            .is_some_and(|entry| entry.is_valid())
    }

    pub(crate) fn mark_mod_unavailable(&mut self, mod_id: u64) {
        self.unavailable_mods.insert(mod_id, Timed::new(()));
    }

    pub(crate) fn invalidate_mod(&mut self, mod_id: u64) {
        self.unavailable_mods.remove(&mod_id);
    }

    pub(crate) fn get_dependencies(&self, mod_id: u64) -> Option<Vec<u64>> {
        self.dependencies
            .get(&mod_id)
            .filter(|entry| entry.is_valid())
            .map(|entry| entry.value.clone())
    }

    pub(crate) fn store_dependencies(&mut self, mod_id: u64, dependencies: Vec<u64>) {
        self.dependencies
            .insert(mod_id, Timed::new(dependencies));
    }
}
