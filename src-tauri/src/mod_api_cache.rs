use std::collections::HashMap;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use crate::modio_api::ModObject;

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

/// Disk-persisted subset of the cache. Only the dependency map is persisted: it
/// is effectively immutable per mod and is the main cold-start request burst.
/// Latest-file-ids and subscriptions are intentionally left out so update
/// detection and subscription state stay fresh after a restart.
#[derive(Serialize, Deserialize, Default)]
pub(crate) struct PersistedCache {
    pub dependencies: HashMap<u64, Vec<u64>>,
}

#[derive(Default)]
pub(crate) struct ApiCache {
    unavailable_mods: HashMap<u64, Timed<()>>,
    dependencies: HashMap<u64, Timed<Vec<u64>>>,
    latest_file_ids: HashMap<u64, Timed<u64>>,
    mods: HashMap<u64, Timed<ModObject>>,
    subscribed_mod_ids: Option<Timed<Vec<u64>>>,
}

impl ApiCache {
    pub(crate) fn clear(&mut self) {
        self.unavailable_mods.clear();
        self.dependencies.clear();
        self.latest_file_ids.clear();
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

    pub(crate) fn invalidate_mod(&mut self, mod_id: u64) {
        self.unavailable_mods.remove(&mod_id);
        self.latest_file_ids.remove(&mod_id);
        self.mods.remove(&mod_id);
    }

    pub(crate) fn get_mod(&self, mod_id: u64) -> Option<ModObject> {
        self.mods
            .get(&mod_id)
            .filter(|entry| entry.is_valid())
            .map(|entry| entry.value.clone())
    }

    pub(crate) fn store_mod(&mut self, mod_: ModObject) {
        self.mods.insert(mod_.id, Timed::new(mod_));
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

    /// Snapshot of the still-valid dependency entries for disk persistence.
    pub(crate) fn dependency_snapshot(&self) -> PersistedCache {
        let dependencies = self
            .dependencies
            .iter()
            .filter(|(_, entry)| entry.is_valid())
            .map(|(mod_id, entry)| (*mod_id, entry.value.clone()))
            .collect();
        PersistedCache { dependencies }
    }

    /// Loads a persisted snapshot, treating restored entries as freshly fetched.
    pub(crate) fn restore_persisted(&mut self, snapshot: PersistedCache) {
        for (mod_id, dependencies) in snapshot.dependencies {
            self.dependencies.entry(mod_id).or_insert_with(|| Timed::new(dependencies));
        }
    }
}
