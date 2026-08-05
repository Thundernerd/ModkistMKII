<script setup lang="ts">
import { onMounted, ref } from "vue";
import { confirm } from "@tauri-apps/plugin-dialog";
import { useModFilters } from "~/composables/useModFilters";
import { invoke } from "~/utils/tauri";

definePageMeta({ layout: "app" });

const {
  mods,
  total,
  loading,
  search,
  modType,
  sort,
  sortDir,
  hasMore,
  loadMore,
  activeCategoryLabel,
  activeCategoryOptions,
  hasActiveFilters,
  hasCategoryTagFilters,
  categoryTagState,
  toggleCategoryTag,
  clearCategoryTags,
  clearFilters,
  initialize,
} = useModFilters();

const {
  checkForUpdatesOnStartup,
  installMod,
  uninstallMod,
  getUiStatus,
  getCanUninstall,
  getInstallError,
  isUninstalling,
  installEnvironmentError,
  updateCount,
  checkingUpdates,
  syncingSubscriptions,
  profileInstallBlocked,
  gameRunning,
  gameRunningMessage,
  refreshInstalled,
  resetSessionSync,
  syncSubscribedModsIfNeeded,
} = useModInstall();

const {
  modioConfigured,
  modioMessage,
  modioStatusChecked,
  refreshModioStatus,
} = useModioStatus();

const refreshing = ref(false);

async function handleInstall(modId: number) {
  await installMod(modId);
}

async function handleUninstall(modId: number, modName: string) {
  const confirmed = await confirm(
    `Remove "${modName}" from your game folder?`,
    { title: "Uninstall mod?", kind: "warning" },
  );
  if (!confirmed) return;
  await uninstallMod(modId);
}

async function refreshMods() {
  if (!modioConfigured.value || refreshing.value) return;
  refreshing.value = true;
  try {
    await invoke("clear_mod_api_cache");
    checkingUpdates.value = true;
    try {
      await Promise.all([
        initialize(),
        refreshInstalled({ force: true }),
      ]);
    } finally {
      checkingUpdates.value = false;
    }
    resetSessionSync();
    await syncSubscribedModsIfNeeded();
  } finally {
    refreshing.value = false;
  }
}

onMounted(async () => {
  await refreshModioStatus();
  if (modioConfigured.value) {
    await Promise.all([initialize(), checkForUpdatesOnStartup()]);
    await syncSubscribedModsIfNeeded();
  }
});
</script>

<template>
  <div class="mods-page">
    <header class="page-header">
      <div class="page-header-row">
        <h1>Mods</h1>
        <button
          type="button"
          class="btn-secondary page-header-action"
          :disabled="
            !modioConfigured ||
            refreshing ||
            loading ||
            checkingUpdates ||
            syncingSubscriptions
          "
          @click="refreshMods"
        >
          {{
            refreshing || loading || checkingUpdates || syncingSubscriptions
              ? "Refreshing…"
              : "Refresh"
          }}
        </button>
      </div>
    </header>

    <div v-if="!modioStatusChecked" class="state mods-loading">
      <span class="spinner" aria-hidden="true" />
      Loading mods…
    </div>

    <p v-else-if="!modioConfigured" class="hint mods-hint">
      {{ modioMessage || "Configure mod.io in .env (see .env.example)." }}
    </p>

    <template v-else>
      <ModFilters
        v-model:search="search"
        v-model:mod-type="modType"
        v-model:sort="sort"
        v-model:sort-dir="sortDir"
        :category-options="activeCategoryOptions"
        :category-label="activeCategoryLabel"
        :has-active-filters="hasActiveFilters"
        :has-category-tag-filters="hasCategoryTagFilters"
        :category-tag-state="categoryTagState"
        @toggle-category-tag="toggleCategoryTag"
        @clear-category-tags="clearCategoryTags"
        @clear-filters="clearFilters"
      />

      <p v-if="installEnvironmentError" class="hint install-hint">
        Installs are unavailable: {{ installEnvironmentError }}
        <NuxtLink to="/settings">Open Settings</NuxtLink>
      </p>

      <p v-else-if="profileInstallBlocked" class="hint install-hint">
        Installs are disabled on the Vanilla profile.
        <NuxtLink to="/settings">Manage profiles</NuxtLink>
      </p>

      <p v-else-if="gameRunning" class="hint install-hint">
        {{ gameRunningMessage ?? "Zeepkist is running. Close the game before installing or updating mods." }}
      </p>

      <p v-else-if="checkingUpdates" class="hint updates-check-hint">
        <span class="spinner" aria-hidden="true" />
        Update check in progress…
      </p>

      <p v-else-if="syncingSubscriptions" class="hint updates-check-hint">
        <span class="spinner" aria-hidden="true" />
        Syncing subscribed mods…
      </p>

      <p
        v-else-if="updateCount > 0"
        class="hint updates-banner"
      >
        {{ updateCount }} installed mod{{ updateCount === 1 ? "" : "s" }}
        {{ updateCount === 1 ? "has" : "have" }} an update available.
        <NuxtLink to="/updates">View updates</NuxtLink>
      </p>

      <p v-if="!loading || mods.length" class="meta mods-count">
        Showing {{ mods.length }} of {{ total }} mods
      </p>
    </template>

    <div v-if="modioConfigured && loading && mods.length === 0" class="state">
      <span class="spinner" aria-hidden="true" />
      Loading mods…
    </div>

    <div
      v-else-if="modioConfigured && mods.length === 0 && !loading"
      class="state"
    >
      No mods found.
    </div>

    <ul v-else-if="mods.length" class="mod-grid">
      <li v-for="mod in mods" :key="mod.id">
        <ModCard
          :mod="mod"
          :install-status="getUiStatus(mod.id)"
          :can-uninstall="getCanUninstall(mod.id)"
          :is-uninstalling="isUninstalling(mod.id)"
          :install-error="getInstallError(mod.id)"
          @install="handleInstall(mod.id)"
          @uninstall="handleUninstall(mod.id, mod.name)"
        />
      </li>
    </ul>

    <footer v-if="hasMore" class="mods-footer">
      <button
        type="button"
        class="btn-secondary"
        :disabled="loading"
        @click="loadMore"
      >
        {{ loading ? "Loading…" : "Load more" }}
      </button>
    </footer>
  </div>
</template>

<style scoped>
.page-header {
  margin-bottom: 1.25rem;
}

.page-header-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 1rem;
}

.page-header-action {
  flex-shrink: 0;
}

.page-header h1 {
  margin: 0;
  font-size: 1.5rem;
  font-weight: 700;
  letter-spacing: -0.02em;
}

.mods-hint {
  padding: 2rem;
  text-align: center;
  border: 1px dashed var(--modio-border);
  border-radius: var(--modio-radius);
  background: var(--modio-surface);
}

.mods-count {
  margin: 0 0 1.25rem;
}

.install-hint {
  margin: 0 0 1rem;
  padding: 0.85rem 1rem;
  border-radius: var(--modio-radius-sm);
  background: var(--modio-surface);
  border: 1px solid var(--modio-border);
}

.updates-check-hint,
.updates-banner {
  display: flex;
  align-items: center;
  gap: 0.65rem;
  margin: 0 0 1rem;
  padding: 0.85rem 1rem;
  border-radius: var(--modio-radius-sm);
  background: var(--modio-surface);
  border: 1px solid var(--modio-border);
}

.updates-banner {
  border-color: rgba(var(--modio-accent-rgb), 0.35);
  background: rgba(var(--modio-accent-rgb), 0.08);
}

.state {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 0.75rem;
  padding: 4rem 1rem;
  color: var(--modio-text-muted);
}

.spinner {
  width: 1.1rem;
  height: 1.1rem;
  border: 2px solid var(--modio-border);
  border-top-color: var(--modio-accent);
  border-radius: 50%;
  animation: spin 0.7s linear infinite;
}

@keyframes spin {
  to {
    transform: rotate(360deg);
  }
}

.mod-grid {
  list-style: none;
  margin: 0;
  padding: 0;
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(15.5rem, 1fr));
  gap: 1rem;
}

.mods-footer {
  display: flex;
  justify-content: center;
  margin-top: 2rem;
}
</style>
