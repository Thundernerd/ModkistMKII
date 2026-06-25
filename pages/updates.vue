<script setup lang="ts">
import { confirm } from "@tauri-apps/plugin-dialog";

definePageMeta({ layout: "app" });

const {
  modsWithUpdates,
  updateCount,
  installEnvironmentError,
  refreshInstalled,
  installMod,
  getUiStatus,
  getInstallError,
  bulkUpdating,
  checkingUpdates,
  updateAllMods,
  profileInstallBlocked,
  gameRunning,
  gameRunningMessage,
} = useModInstall();

const loading = ref(true);
const pageError = ref("");
const bulkResult = ref<{ updated: number[]; failed: number[] } | null>(null);

async function loadUpdates() {
  loading.value = true;
  pageError.value = "";
  bulkResult.value = null;
  try {
    await refreshInstalled();
  } catch (error) {
    pageError.value = error instanceof Error ? error.message : String(error);
  } finally {
    loading.value = false;
  }
}

async function handleInstall(modId: number) {
  bulkResult.value = null;
  await installMod(modId);
}

async function handleUpdateAll() {
  if (updateCount.value === 0) return;

  const confirmed = await confirm(
    `Download and install updates for ${updateCount.value} mod${updateCount.value === 1 ? "" : "s"}?`,
    { title: "Update all mods?", kind: "info" },
  );
  if (!confirmed) return;

  bulkResult.value = null;
  pageError.value = "";
  const result = await updateAllMods();
  bulkResult.value = result;

  if (result.failed.length > 0 && result.updated.length === 0) {
    pageError.value = "Could not update any mods. Check the errors below and try again.";
  }
}

onMounted(loadUpdates);
</script>

<template>
  <div class="updates-page">
    <header class="page-header">
      <div class="page-header-row">
        <div>
          <h1>Updates</h1>
          <p class="page-subtitle">
            Installed mods with a newer version on mod.io.
          </p>
        </div>

        <button
          v-if="!loading && updateCount > 0 && !profileInstallBlocked && !gameRunning"
          type="button"
          class="btn-primary update-all-btn"
          :disabled="bulkUpdating || checkingUpdates"
          @click="handleUpdateAll"
        >
          <span v-if="bulkUpdating" class="spinner" aria-hidden="true" />
          {{ bulkUpdating ? "Updating…" : `Update all (${updateCount})` }}
        </button>
      </div>
    </header>

    <p v-if="installEnvironmentError" class="hint install-hint">
      {{ installEnvironmentError }}
      <NuxtLink to="/settings">Check Settings</NuxtLink>
    </p>

    <p v-else-if="profileInstallBlocked" class="hint install-hint">
      Updates are disabled on the Vanilla profile.
      <NuxtLink to="/settings">Manage profiles</NuxtLink>
    </p>

    <p v-else-if="gameRunning" class="hint install-hint">
      {{ gameRunningMessage ?? "Zeepkist is running. Close the game before installing or updating mods." }}
    </p>

    <p v-if="pageError" class="error">{{ pageError }}</p>

    <p
      v-if="bulkResult && bulkResult.updated.length > 0"
      class="hint bulk-result"
    >
      Updated {{ bulkResult.updated.length }} mod{{
        bulkResult.updated.length === 1 ? "" : "s"
      }}<span v-if="bulkResult.failed.length > 0">
        · {{ bulkResult.failed.length }} failed</span
      >.
    </p>

    <div v-if="loading || checkingUpdates" class="state">
      <span class="spinner" aria-hidden="true" />
      Checking for updates…
    </div>

    <div
      v-else-if="!installEnvironmentError && updateCount === 0"
      class="hint empty-state"
    >
      All installed mods are up to date.
      <NuxtLink to="/installed">View installed mods</NuxtLink>
    </div>

    <ul v-else-if="modsWithUpdates.length" class="updates-list">
      <li
        v-for="mod in modsWithUpdates"
        :key="`${mod.modId}-${mod.fileId}`"
      >
        <article class="updates-card">
          <NuxtLink :to="`/mods/${mod.modId}`" class="updates-card-link">
            <div class="updates-thumb">
              <img
                v-if="mod.logoUrl"
                :src="mod.logoUrl"
                :alt="`${mod.name} logo`"
                loading="lazy"
              />
              <div v-else class="updates-thumb-fallback" />
            </div>

            <div class="updates-info">
              <h2>{{ mod.name }}</h2>
              <p class="updates-summary">{{ mod.summary }}</p>
              <p class="updates-meta">
                Installed file {{ mod.fileId }}
                <span v-if="mod.latestFileId">
                  · Latest {{ mod.latestFileId }}
                </span>
              </p>
            </div>
          </NuxtLink>

          <div class="updates-actions">
            <ModInstallButton
              :mod-id="mod.modId"
              :status="getUiStatus(mod.modId)"
              :error="getInstallError(mod.modId)"
              @install="handleInstall(mod.modId)"
            />
          </div>
        </article>
      </li>
    </ul>
  </div>
</template>

<style scoped>
.page-header {
  margin-bottom: 1.25rem;
}

.page-header-row {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 1rem;
}

.page-header h1 {
  margin: 0;
  font-size: 1.5rem;
  font-weight: 700;
  letter-spacing: -0.02em;
}

.page-subtitle {
  margin: 0.35rem 0 0;
  color: var(--modio-text-muted);
  font-size: 0.92rem;
}

.update-all-btn {
  display: inline-flex;
  align-items: center;
  gap: 0.45rem;
  flex-shrink: 0;
}

.install-hint,
.empty-state,
.bulk-result {
  margin-bottom: 1rem;
  padding: 1rem 1.1rem;
  border-radius: var(--modio-radius);
  border: 1px dashed var(--modio-border);
  background: var(--modio-surface);
}

.bulk-result {
  border-style: solid;
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

.updates-list {
  list-style: none;
  margin: 0;
  padding: 0;
  display: flex;
  flex-direction: column;
  gap: 0.85rem;
}

.updates-card {
  display: grid;
  grid-template-columns: minmax(0, 1fr) auto;
  gap: 1rem;
  align-items: center;
  padding: 0.9rem 1rem;
  border-radius: var(--modio-radius);
  border: 1px solid rgba(var(--modio-accent-rgb), 0.28);
  background: var(--modio-surface);
}

.updates-card-link {
  display: grid;
  grid-template-columns: 6.5rem minmax(0, 1fr);
  gap: 0.9rem;
  align-items: center;
  color: inherit;
  text-decoration: none;
}

.updates-thumb {
  aspect-ratio: 16 / 9;
  border-radius: var(--modio-radius-sm);
  overflow: hidden;
  background: var(--modio-surface-raised);
}

.updates-thumb img {
  width: 100%;
  height: 100%;
  object-fit: cover;
  display: block;
}

.updates-thumb-fallback {
  width: 100%;
  height: 100%;
  background: linear-gradient(
    135deg,
    var(--modio-surface-raised),
    var(--modio-surface-hover)
  );
}

.updates-info h2 {
  margin: 0;
  font-size: 1rem;
  font-weight: 600;
}

.updates-summary {
  margin: 0.35rem 0 0;
  font-size: 0.85rem;
  color: var(--modio-text-subtle);
  display: -webkit-box;
  -webkit-line-clamp: 2;
  -webkit-box-orient: vertical;
  overflow: hidden;
}

.updates-meta {
  margin: 0.4rem 0 0;
  font-size: 0.78rem;
  color: var(--modio-text-muted);
}

.updates-actions {
  min-width: 8.5rem;
}

@media (max-width: 760px) {
  .page-header-row {
    flex-direction: column;
  }

  .updates-card {
    grid-template-columns: 1fr;
  }

  .updates-actions {
    min-width: 0;
  }
}
</style>
