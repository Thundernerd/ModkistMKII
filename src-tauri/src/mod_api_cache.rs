use std::collections::HashMap;
use std::time::{Duration, Instant};

use crate::modio_api::{ModObject, Modfile};

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
    failed_dependency_fetches: HashMap<u64, Timed<()>>,
    dependencies: HashMap<u64, Timed<Vec<u64>>>,
    latest_file_ids: HashMap<u64, Timed<u64>>,
    mod_files: HashMap<u64, Timed<Vec<Modfile>>>,
    mods: HashMap<u64, Timed<ModObject>>,
    subscribed_mod_ids: Option<Timed<Vec<u64>>>,
}

impl ApiCache {
    pub(crate) fn clear(&mut self) {
        self.unavailable_mods.clear();
        self.failed_dependency_fetches.clear();
        self.dependencies.clear();
        self.latest_file_ids.clear();
        self.mod_files.clear();
        self.mods.clear();
        self.subscribed_mod_ids = None;
    }

    pub(crate) fn is_mod_unavailable(&self, mod_id: u64) -> bool {
        self.unavailable_mods
            .get(&mod_id)
            .is_some_and(|entry| entry.is_valid())
    }

    pub(crate) fn mark_mod_unavailable(&mut self, mod_id: u64) {
        self.unavailable_mods.insert(mod_id, Timed::new(()));
    }

    pub(crate) fn is_dependency_fetch_failed(&self, mod_id: u64) -> bool {
        self.failed_dependency_fetches
            .get(&mod_id)
            .is_some_and(|entry| entry.is_valid())
    }

    pub(crate) fn mark_dependency_fetch_failed(&mut self, mod_id: u64) {
        self.failed_dependency_fetches.insert(mod_id, Timed::new(()));
    }

    pub(crate) fn invalidate_mod(&mut self, mod_id: u64) {
        self.unavailable_mods.remove(&mod_id);
        self.failed_dependency_fetches.remove(&mod_id);
        self.latest_file_ids.remove(&mod_id);
        self.mod_files.remove(&mod_id);
        self.mods.remove(&mod_id);
    }

    pub(crate) fn get_mod(&self, mod_id: u64) -> Option<ModObject> {
        self.mods
            .get(&mod_id)
            .filter(|entry| entry.is_valid())
            .map(|entry| entry.value.clone())
    }

    pub(crate) fn store_mod(&mut self, mod_: ModObject) {
        let mod_id = mod_.id;
        let merged = if let Some(entry) = self.mods.get(&mod_id).filter(|entry| entry.is_valid()) {
            let mut merged = mod_;
            if merged.description.as_ref().is_none_or(|value| value.is_empty()) {
                merged.description = entry.value.description.clone();
            }
            merged
        } else {
            mod_
        };
        self.mods.insert(mod_id, Timed::new(merged));
    }

    pub(crate) fn get_mod_files(&self, mod_id: u64) -> Option<Vec<Modfile>> {
        self.mod_files
            .get(&mod_id)
            .filter(|entry| entry.is_valid())
            .map(|entry| entry.value.clone())
    }

    pub(crate) fn store_mod_files(&mut self, mod_id: u64, files: Vec<Modfile>) {
        self.mod_files.insert(mod_id, Timed::new(files));
    }

    pub(crate) fn get_latest_file_id(&self, mod_id: u64) -> Option<u64> {
        self.latest_file_ids
            .get(&mod_id)
            .filter(|entry| entry.is_valid())
            .map(|entry| entry.value)
    }

    pub(crate) fn store_latest_file_id(&mut self, mod_id: u64, file_id: u64) {
        self.latest_file_ids.insert(mod_id, Timed::new(file_id));
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

    pub(crate) fn get_subscribed_mod_ids(&self) -> Option<Vec<u64>> {
        self.subscribed_mod_ids
            .as_ref()
            .filter(|entry| entry.is_valid())
            .map(|entry| entry.value.clone())
    }

    pub(crate) fn store_subscribed_mod_ids(&mut self, mod_ids: Vec<u64>) {
        self.subscribed_mod_ids = Some(Timed::new(mod_ids));
    }

    pub(crate) fn add_subscribed_mod_id(&mut self, mod_id: u64) {
        let mut ids = self.get_subscribed_mod_ids().unwrap_or_default();
        if ids.binary_search(&mod_id).is_err() {
            ids.push(mod_id);
            ids.sort_unstable();
            self.store_subscribed_mod_ids(ids);
        }
    }

    pub(crate) fn remove_subscribed_mod_id(&mut self, mod_id: u64) {
        let Some(mut ids) = self.get_subscribed_mod_ids() else {
            return;
        };
        if let Ok(index) = ids.binary_search(&mod_id) {
            ids.remove(index);
            self.store_subscribed_mod_ids(ids);
        }
    }
}
