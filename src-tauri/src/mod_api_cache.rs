use std::collections::HashMap;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use crate::mod_ids::{FileId, ModId};
use crate::modio_api::{ModObject, Modfile};

const CACHE_TTL: Duration = Duration::from_secs(300);
/// Dependency graphs are effectively immutable per mod (unlike mod metadata,
/// files or subscription state), so they can be cached far longer without
/// going stale — this cuts most of the per-installed-mod dependency requests
/// that repeat every time the 5-minute default TTL expires within a session.
const DEPENDENCY_CACHE_TTL: Duration = Duration::from_secs(24 * 60 * 60);

struct Timed<T> {
    value: T,
    fetched_at: Instant,
    ttl: Duration,
}

impl<T: Clone> Timed<T> {
    fn new(value: T) -> Self {
        Self::with_ttl(value, CACHE_TTL)
    }

    fn with_ttl(value: T, ttl: Duration) -> Self {
        Self {
            value,
            fetched_at: Instant::now(),
            ttl,
        }
    }

    fn is_valid(&self) -> bool {
        self.fetched_at.elapsed() < self.ttl
    }
}

/// Disk-persisted subset of the cache. Only the dependency map is persisted: it
/// is effectively immutable per mod and is the main cold-start request burst.
/// Latest-file-ids and subscriptions are intentionally left out so update
/// detection and subscription state stay fresh after a restart.
#[derive(Serialize, Deserialize, Default)]
pub(crate) struct PersistedCache {
    pub dependencies: HashMap<ModId, Vec<ModId>>,
    #[serde(default)]
    pub dependency_fetch_failures: Vec<ModId>,
}

#[derive(Default)]
pub(crate) struct ApiCache {
    unavailable_mods: HashMap<ModId, Timed<()>>,
    /// `(mod_id, file_id)` pairs mod.io reported as deleted (404).
    deleted_mod_files: HashMap<(ModId, FileId), Timed<()>>,
    failed_dependency_fetches: HashMap<ModId, Timed<()>>,
    dependencies: HashMap<ModId, Timed<Vec<ModId>>>,
    latest_file_ids: HashMap<ModId, Timed<FileId>>,
    mod_files: HashMap<ModId, Timed<Vec<Modfile>>>,
    mods: HashMap<ModId, Timed<ModObject>>,
    subscribed_mod_ids: Option<Timed<Vec<ModId>>>,
}

impl ApiCache {
    pub(crate) fn clear(&mut self) {
        self.unavailable_mods.clear();
        self.deleted_mod_files.clear();
        self.failed_dependency_fetches.clear();
        self.dependencies.clear();
        self.latest_file_ids.clear();
        self.mod_files.clear();
        self.mods.clear();
        self.subscribed_mod_ids = None;
    }

    pub(crate) fn is_mod_unavailable(&self, mod_id: ModId) -> bool {
        self.unavailable_mods
            .get(&mod_id)
            .is_some_and(|entry| entry.is_valid())
    }

    pub(crate) fn mark_mod_unavailable(&mut self, mod_id: ModId) {
        self.unavailable_mods.insert(mod_id, Timed::new(()));
    }

    pub(crate) fn is_mod_file_deleted(&self, mod_id: ModId, file_id: FileId) -> bool {
        self.deleted_mod_files
            .get(&(mod_id, file_id))
            .is_some_and(|entry| entry.is_valid())
    }

    pub(crate) fn mark_mod_file_deleted(&mut self, mod_id: ModId, file_id: FileId) {
        self.deleted_mod_files.insert((mod_id, file_id), Timed::new(()));
    }

    pub(crate) fn is_dependency_fetch_failed(&self, mod_id: ModId) -> bool {
        self.failed_dependency_fetches
            .get(&mod_id)
            .is_some_and(|entry| entry.is_valid())
    }

    pub(crate) fn mark_dependency_fetch_failed(&mut self, mod_id: ModId) {
        self.failed_dependency_fetches.insert(mod_id, Timed::new(()));
    }

    pub(crate) fn invalidate_mod(&mut self, mod_id: ModId) {
        self.unavailable_mods.remove(&mod_id);
        self.deleted_mod_files.retain(|(deleted_mod_id, _), _| *deleted_mod_id != mod_id);
        self.failed_dependency_fetches.remove(&mod_id);
        self.latest_file_ids.remove(&mod_id);
        self.mod_files.remove(&mod_id);
        self.mods.remove(&mod_id);
    }

    pub(crate) fn get_mod(&self, mod_id: ModId) -> Option<ModObject> {
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

    pub(crate) fn get_mod_files(&self, mod_id: ModId) -> Option<Vec<Modfile>> {
        self.mod_files
            .get(&mod_id)
            .filter(|entry| entry.is_valid())
            .map(|entry| entry.value.clone())
    }

    pub(crate) fn store_mod_files(&mut self, mod_id: ModId, files: Vec<Modfile>) {
        self.mod_files.insert(mod_id, Timed::new(files));
    }

    pub(crate) fn get_latest_file_id(&self, mod_id: ModId) -> Option<FileId> {
        self.latest_file_ids
            .get(&mod_id)
            .filter(|entry| entry.is_valid())
            .map(|entry| entry.value)
    }

    pub(crate) fn store_latest_file_id(&mut self, mod_id: ModId, file_id: FileId) {
        self.latest_file_ids.insert(mod_id, Timed::new(file_id));
    }

    pub(crate) fn get_dependencies(&self, mod_id: ModId) -> Option<Vec<ModId>> {
        self.dependencies
            .get(&mod_id)
            .filter(|entry| entry.is_valid())
            .map(|entry| entry.value.clone())
    }

    pub(crate) fn store_dependencies(&mut self, mod_id: ModId, dependencies: Vec<ModId>) {
        self.dependencies
            .insert(mod_id, Timed::with_ttl(dependencies, DEPENDENCY_CACHE_TTL));
    }

    pub(crate) fn get_subscribed_mod_ids(&self) -> Option<Vec<ModId>> {
        self.subscribed_mod_ids
            .as_ref()
            .filter(|entry| entry.is_valid())
            .map(|entry| entry.value.clone())
    }

    pub(crate) fn store_subscribed_mod_ids(&mut self, mod_ids: Vec<ModId>) {
        self.subscribed_mod_ids = Some(Timed::new(mod_ids));
    }

    pub(crate) fn add_subscribed_mod_id(&mut self, mod_id: ModId) {
        let mut ids = self.get_subscribed_mod_ids().unwrap_or_default();
        if ids.binary_search(&mod_id).is_err() {
            ids.push(mod_id);
            ids.sort_unstable();
            self.store_subscribed_mod_ids(ids);
        }
    }

    pub(crate) fn remove_subscribed_mod_id(&mut self, mod_id: ModId) {
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
        let dependency_fetch_failures = self
            .failed_dependency_fetches
            .iter()
            .filter(|(_, entry)| entry.is_valid())
            .map(|(mod_id, _)| *mod_id)
            .collect();
        PersistedCache {
            dependencies,
            dependency_fetch_failures,
        }
    }

    /// Loads a persisted snapshot, treating restored entries as freshly fetched.
    pub(crate) fn restore_persisted(&mut self, snapshot: PersistedCache) {
        for (mod_id, dependencies) in snapshot.dependencies {
            self.dependencies
                .entry(mod_id)
                .or_insert_with(|| Timed::with_ttl(dependencies, DEPENDENCY_CACHE_TTL));
        }
        for mod_id in snapshot.dependency_fetch_failures {
            self.failed_dependency_fetches
                .entry(mod_id)
                .or_insert_with(|| Timed::new(()));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mod_ids::ModId;

    #[test]
    fn a_persisted_cache_file_with_plain_number_ids_round_trips_unchanged() {
        let stored = r#"{"dependencies":{"12":[34,56]},"dependency_fetch_failures":[78]}"#;

        let cache: PersistedCache = serde_json::from_str(stored).unwrap();

        assert_eq!(cache.dependencies[&ModId(12)], vec![ModId(34), ModId(56)]);
        assert_eq!(cache.dependency_fetch_failures, vec![ModId(78)]);
        assert_eq!(serde_json::to_string(&cache).unwrap(), stored);
    }
}
