<script setup lang="ts">
import { confirm } from "@tauri-apps/plugin-dialog";
import { formatRelativeAgo } from "~/utils/formatRelative";
import type { InstallHistoryEntry } from "~/composables/useInstallHistory";

definePageMeta({ layout: "app" });

const {
  modsWithUpdates,
  updateCount,
  installEnvironmentError,
  installReady,
  checkingUpdates,
  ensureInstalledModsLoaded,
  installMod,
  getUiStatus,
  getInstallError,
  bulkUpdating,
  updateAllMods,
  profileInstallBlocked,
  gameRunning,
  gameRunningMessage,
} = useModInstall();

const { history } = useInstallHistory();

const loading = ref(!installReady.value);
const pageError = ref("");

async function loadUpdates() {
  if (installReady.value) {
    loading.value = false;
    return;
  }

  loading.value = true;
  pageError.value = "";
  try {
    await ensureInstalledModsLoaded();
  } catch (error) {
    pageError.value = error instanceof Error ? error.message : String(error);
  } finally {
    loading.value = false;
  }
}

async function handleInstall(modId: number) {
  await installMod(modId);
}

async function handleUpdateAll() {
  if (updateCount.value === 0) return;

  const confirmed = await confirm(
    `Download and install updates for ${updateCount.value} mod${updateCount.value === 1 ? "" : "s"}?`,
    { title: "Update all mods?", kind: "info" },
  );
  if (!confirmed) return;

  await updateAllMods();
}

function actionLabel(action: InstallHistoryEntry["action"]) {
  return action === "updated" ? "Updated" : "Installed";
}

function historyMeta(entry: InstallHistoryEntry) {
  const parts: string[] = [
    formatRelativeAgo(new Date(entry.occurredAt).toISOString()),
  ];

  if (entry.versionLabel) {
    parts.push(entry.versionLabel);
  } else if (entry.fileId != null) {
    parts.push(`file ${entry.fileId}`);
  }

  if (entry.dependencyCount > 0) {
    parts.push(
      `+${entry.dependencyCount} dependenc${entry.dependencyCount === 1 ? "y" : "ies"}`,
    );
  }

  return parts.filter(Boolean).join(" · ");
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
            Pending updates and recent installs from this session.
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
      <NuxtLink to="/settings">Open Settings</NuxtLink>
    </p>

    <p v-else-if="profileInstallBlocked" class="hint install-hint">
      Updates are disabled on the Vanilla profile.
      <NuxtLink to="/settings">Manage profiles</NuxtLink>
    </p>

    <p v-else-if="gameRunning" class="hint install-hint">
      {{ gameRunningMessage ?? "Zeepkist is running. Close the game before installing or updating mods." }}
    </p>

    <p v-if="pageError" class="error">{{ pageError }}</p>

    <div v-if="loading || checkingUpdates" class="state">
      <span class="spinner" aria-hidden="true" />
      Update check in progress…
    </div>

    <template v-else>
      <section class="pending-section" aria-label="Pending updates">
        <div
          v-if="!installEnvironmentError && updateCount === 0"
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
                  <p
                    v-if="mod.version || mod.latestVersion"
                    class="updates-meta"
                  >
                    <template v-if="mod.version">Installed {{ mod.version }}</template>
                    <span v-if="mod.latestVersion">
                      <template v-if="mod.version"> · </template>Latest {{ mod.latestVersion }}
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
      </section>
    </template>

    <section
      v-if="!loading"
      class="history-section"
      aria-label="Recent activity"
    >
      <header class="history-header">
        <h2>Recent activity</h2>
        <p class="history-subtitle">
          Session only — cleared when Modkist quits.
        </p>
      </header>

      <div v-if="history.length === 0" class="hint empty-state">
        No installs or updates yet this session.
      </div>

      <ul v-else class="updates-list">
        <li v-for="entry in history" :key="entry.id">
          <article class="history-card">
            <NuxtLink :to="`/mods/${entry.modId}`" class="updates-card-link">
              <div class="updates-thumb">
                <img
                  v-if="entry.logoUrl"
                  :src="entry.logoUrl"
                  :alt="`${entry.name} logo`"
                  loading="lazy"
                />
                <div v-else class="updates-thumb-fallback" />
              </div>

              <div class="updates-info">
                <div class="history-title-row">
                  <h3>{{ entry.name }}</h3>
                  <span
                    class="ui-pill history-badge"
                    :data-action="entry.action"
                  >
                    <span class="ui-pill-text">{{ actionLabel(entry.action) }}</span>
                  </span>
                </div>
                <p class="updates-meta">{{ historyMeta(entry) }}</p>
              </div>
            </NuxtLink>
          </article>
        </li>
      </ul>
    </section>
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
.empty-state {
  margin-bottom: 1rem;
  padding: 1rem 1.1rem;
  border-radius: var(--modio-radius);
  border: 1px dashed var(--modio-border);
  background: var(--modio-surface);
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

.pending-section {
  margin-bottom: 0.25rem;
}

.history-section {
  margin-top: 1.75rem;
  padding-top: 1.5rem;
  border-top: 1px solid var(--modio-border);
}

.history-header {
  margin-bottom: 1rem;
}

.history-header h2 {
  margin: 0;
  font-size: 1.15rem;
  font-weight: 650;
  letter-spacing: -0.01em;
}

.history-subtitle {
  margin: 0.3rem 0 0;
  color: var(--modio-text-muted);
  font-size: 0.85rem;
}

.updates-list {
  list-style: none;
  margin: 0;
  padding: 0;
  display: flex;
  flex-direction: column;
  gap: 0.85rem;
}

.updates-card,
.history-card {
  display: grid;
  grid-template-columns: minmax(0, 1fr) auto;
  gap: 1rem;
  align-items: center;
  padding: 0.9rem 1rem;
  border-radius: var(--modio-radius);
  background: var(--modio-surface);
}

.updates-card {
  border: 1px solid rgba(var(--modio-accent-rgb), 0.28);
}

.history-card {
  grid-template-columns: minmax(0, 1fr);
  border: 1px solid var(--modio-border);
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

.updates-info h2,
.updates-info h3 {
  margin: 0;
  font-size: 1rem;
  font-weight: 600;
}

.history-title-row {
  display: flex;
  align-items: center;
  gap: 0.55rem;
  flex-wrap: wrap;
}

.history-badge {
  border-radius: var(--modio-radius-sm);
  letter-spacing: 0.01em;
  background: var(--modio-surface-raised);
  color: var(--modio-text-muted);
  border: 1px solid var(--modio-border);
}

.history-badge[data-action="updated"] {
  color: var(--modio-accent);
  border-color: rgba(var(--modio-accent-rgb), 0.35);
  background: rgba(var(--modio-accent-rgb), 0.1);
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
