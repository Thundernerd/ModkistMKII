<script setup lang="ts">
import { onMounted, ref } from "vue";
import { confirm } from "@tauri-apps/plugin-dialog";
import { invoke } from "~/utils/tauri";
import { useModFilters } from "~/composables/useModFilters";

definePageMeta({ layout: "app" });

interface ModioStatus {
  configured: boolean;
  message?: string;
}

const {
  mods,
  total,
  loading,
  error,
  search,
  modType,
  categoryTags,
  sort,
  sortDir,
  hasMore,
  loadMore,
  activeCategoryLabel,
  activeCategoryOptions,
  hasActiveFilters,
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
  syncSubscriptionError,
  profileInstallBlocked,
  gameRunning,
  gameRunningMessage,
  syncSubscribedModsIfNeeded,
} = useModInstall();

const modioConfigured = ref(false);
const modioMessage = ref("");

async function checkModioStatus() {
  const status = await invoke<ModioStatus>("modio_status");
  modioConfigured.value = status.configured;
  modioMessage.value = status.message ?? "";
}

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

onMounted(async () => {
  await checkModioStatus();
  if (modioConfigured.value) {
    await Promise.all([initialize(), checkForUpdatesOnStartup()]);
    await syncSubscribedModsIfNeeded();
  }
});
</script>

<template>
  <div class="mods-page">
    <header class="page-header">
      <h1>Mods</h1>
    </header>

    <p v-if="!modioConfigured" class="hint mods-hint">
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
        :selected-category-tags="categoryTags"
        @toggle-category-tag="toggleCategoryTag"
        @clear-category-tags="clearCategoryTags"
        @clear-filters="clearFilters"
      />

      <p v-if="installEnvironmentError" class="hint install-hint">
        Installs are unavailable: {{ installEnvironmentError }}
        <NuxtLink to="/settings">Check Settings</NuxtLink>
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
        Checking installed mods for updates…
      </p>

      <p v-else-if="syncingSubscriptions" class="hint updates-check-hint">
        <span class="spinner" aria-hidden="true" />
        Syncing subscribed mods…
      </p>

      <p v-else-if="syncSubscriptionError" class="hint sync-error-hint">
        Subscription sync failed: {{ syncSubscriptionError }}
        <span class="sync-error-detail">
          Subscription sync uses your mod.io login (OAuth). If rate limited, wait about a minute and try again.
        </span>
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

    <p v-if="error" class="error mods-error">{{ error }}</p>

    <div v-if="loading && mods.length === 0" class="state">
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
  border-color: rgba(7, 193, 216, 0.35);
  background: rgba(7, 193, 216, 0.08);
}

.sync-error-hint {
  margin: 0 0 1rem;
  padding: 0.85rem 1rem;
  border-radius: var(--modio-radius-sm);
  background: rgba(248, 113, 113, 0.08);
  border: 1px solid rgba(248, 113, 113, 0.35);
  color: var(--modio-danger);
}

.sync-error-detail {
  display: block;
  margin-top: 0.35rem;
  font-size: 0.82rem;
  color: var(--modio-text-muted);
}

.mods-error {
  margin-bottom: 1rem;
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
