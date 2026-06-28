<script setup lang="ts">
definePageMeta({ layout: "app" });

const {
  mods,
  loading,
  error,
  refreshFailedSyncMods,
  setIgnored,
  unsubscribe,
} = useFailedSyncMods();

const actionErrors = ref<Record<number, string>>({});
const unsubscribingIds = ref<Set<number>>(new Set());
const ignoringIds = ref<Set<number>>(new Set());

function setActionError(modId: number, message: string) {
  actionErrors.value = { ...actionErrors.value, [modId]: message };
}

function clearActionError(modId: number) {
  if (!actionErrors.value[modId]) return;
  const { [modId]: _removed, ...rest } = actionErrors.value;
  actionErrors.value = rest;
}

function setUnsubscribing(modId: number, active: boolean) {
  const next = new Set(unsubscribingIds.value);
  if (active) {
    next.add(modId);
  } else {
    next.delete(modId);
  }
  unsubscribingIds.value = next;
}

function setIgnoring(modId: number, active: boolean) {
  const next = new Set(ignoringIds.value);
  if (active) {
    next.add(modId);
  } else {
    next.delete(modId);
  }
  ignoringIds.value = next;
}

async function handleIgnoreToggle(modId: number, ignored: boolean) {
  clearActionError(modId);
  setIgnoring(modId, true);
  try {
    await setIgnored(modId, ignored);
  } catch (err) {
    setActionError(
      modId,
      err instanceof Error ? err.message : String(err),
    );
  } finally {
    setIgnoring(modId, false);
  }
}

async function handleUnsubscribe(modId: number) {
  clearActionError(modId);
  setUnsubscribing(modId, true);
  try {
    await unsubscribe(modId);
  } catch (err) {
    setActionError(
      modId,
      err instanceof Error ? err.message : String(err),
    );
  } finally {
    setUnsubscribing(modId, false);
  }
}

const FAILED_SYNC_ERROR_LABELS: Record<string, string> = {
  install_order: "Could not resolve dependencies",
  dependency: "Dependency could not be installed",
  install_state: "Could not check mod status",
  install: "Install failed",
  rate_limit: "Rate limited",
  auth: "Private or not accessible",
  unavailable: "Mod unavailable",
  game_running: "Game was running",
  profile_blocked: "Wrong profile",
  unknown: "Sync failed",
  other: "Sync failed",
};

function failedSyncErrorLabel(errorType: string) {
  return FAILED_SYNC_ERROR_LABELS[errorType] ?? "Sync failed";
}

function failedModLabel(entry: { modId: number; modName?: string }) {
  return entry.modName ?? `Mod ID ${entry.modId}`;
}

onMounted(refreshFailedSyncMods);
</script>

<template>
  <div class="page">
    <header class="page-header">
      <NuxtLink to="/settings" class="back-link">← Settings</NuxtLink>
      <h1>Sync failures</h1>
      <p class="hint page-desc">
        Subscribed mods that could not be fully synced. Ignored mods are
        skipped on future syncs.
      </p>
    </header>

    <p v-if="error" class="error feedback">{{ error }}</p>

    <div v-if="loading" class="state">
      <span class="spinner" aria-hidden="true" />
      Loading sync failures…
    </div>

    <p v-else-if="mods.length === 0" class="hint empty-state">
      No sync failures.
    </p>

    <ul v-else class="failed-list">
      <li v-for="entry in mods" :key="entry.modId" class="failed-item panel">
        <div class="failed-main">
          <span class="failed-id">{{ failedModLabel(entry) }}</span>
          <p v-if="entry.modName" class="failed-id-sub">Mod ID {{ entry.modId }}</p>
          <p class="failed-error-type">
            {{ failedSyncErrorLabel(entry.errorType) }}
          </p>
          <p v-if="entry.errorDetail" class="failed-error-detail">
            {{ entry.errorDetail }}
          </p>
          <p v-if="actionErrors[entry.modId]" class="item-error">
            {{ actionErrors[entry.modId] }}
          </p>
        </div>

        <div class="failed-actions">
          <label class="ignore-toggle">
            <span class="ignore-label">Ignore</span>
            <button
              type="button"
              class="setting-toggle"
              role="switch"
              :aria-checked="entry.ignored"
              :aria-label="`Ignore ${failedModLabel(entry)}`"
              :disabled="ignoringIds.has(entry.modId)"
              @click="handleIgnoreToggle(entry.modId, !entry.ignored)"
            >
              <span class="setting-toggle-track" :class="{ on: entry.ignored }">
                <span class="setting-toggle-thumb" />
              </span>
            </button>
          </label>

          <div class="action-group">
            <NuxtLink
              :to="`/mods/${entry.modId}`"
              class="action-group-item"
            >
              View
            </NuxtLink>
            <button
              type="button"
              class="action-group-item action-group-item-danger"
              :disabled="unsubscribingIds.has(entry.modId)"
              @click="handleUnsubscribe(entry.modId)"
            >
              <span
                v-if="unsubscribingIds.has(entry.modId)"
                class="spinner"
                aria-hidden="true"
              />
              {{ unsubscribingIds.has(entry.modId) ? "Unsubscribing…" : "Unsubscribe" }}
            </button>
          </div>
        </div>
      </li>
    </ul>
  </div>
</template>

<style scoped>
.page {
  width: 100%;
}

.page-header {
  margin-bottom: 1.5rem;
}

.back-link {
  display: inline-block;
  margin-bottom: 0.75rem;
  color: var(--modio-text-muted);
  font-size: 0.88rem;
  text-decoration: none;
}

.back-link:hover {
  color: var(--modio-accent);
}

.page-header h1 {
  margin: 0 0 0.35rem;
  font-size: 1.5rem;
  font-weight: 700;
  letter-spacing: -0.02em;
}

.page-desc {
  margin: 0;
}

.feedback {
  margin: 0 0 1rem;
}

.empty-state {
  margin: 0;
  padding: 1rem 1.1rem;
  border-radius: var(--modio-radius);
  border: 1px dashed var(--modio-border);
  background: var(--modio-surface);
}

.state {
  display: flex;
  align-items: center;
  gap: 0.75rem;
  padding: 2rem 0;
  color: var(--modio-text-muted);
}

.failed-list {
  list-style: none;
  margin: 0;
  padding: 0;
  display: flex;
  flex-direction: column;
  gap: 0.75rem;
}

.failed-item {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 1rem;
  padding: 0.9rem 1rem;
  border-radius: var(--modio-radius);
  background: var(--modio-surface);
  border: 1px solid var(--modio-border);
  box-shadow: var(--modio-shadow);
}

.failed-main {
  min-width: 0;
}

.failed-id {
  font-weight: 600;
}

.failed-id-sub {
  margin: 0.2rem 0 0;
  font-size: 0.82rem;
  color: var(--modio-text-muted);
  font-variant-numeric: tabular-nums;
}

.failed-error-type {
  margin: 0.35rem 0 0;
  font-size: 0.88rem;
  font-weight: 500;
  color: var(--modio-text);
}

.failed-error-detail {
  margin: 0.25rem 0 0;
  font-size: 0.82rem;
  color: var(--modio-text-muted);
  line-height: 1.45;
}

.item-error {
  margin: 0.35rem 0 0;
  font-size: 0.82rem;
  color: var(--modio-danger);
}

.failed-actions {
  display: flex;
  align-items: center;
  gap: 0.75rem;
  flex-shrink: 0;
}

.action-group {
  display: inline-flex;
  align-items: stretch;
  border: 1px solid var(--modio-border);
  border-radius: var(--modio-radius-sm);
  overflow: hidden;
  background: var(--modio-surface-raised);
}

.action-group-item {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: 0.45rem;
  padding: 0.55em 0.85em;
  border: none;
  border-right: 1px solid var(--modio-border);
  background: transparent;
  color: var(--modio-text);
  font-size: 0.88rem;
  font-weight: 500;
  font-family: inherit;
  text-decoration: none;
  white-space: nowrap;
  cursor: pointer;
}

.action-group-item:last-child {
  border-right: none;
}

.action-group-item:hover:not(:disabled) {
  background: var(--modio-surface-hover);
  color: var(--modio-text);
}

.action-group-item:disabled {
  opacity: 0.6;
  cursor: not-allowed;
}

.action-group-item-danger {
  color: var(--modio-danger);
}

.action-group-item-danger:hover:not(:disabled) {
  background: rgba(248, 113, 113, 0.08);
  color: var(--modio-danger);
}

.ignore-toggle {
  display: flex;
  align-items: center;
  gap: 0.5rem;
}

.ignore-label {
  font-size: 0.84rem;
  color: var(--modio-text-muted);
}

.setting-toggle {
  padding: 0;
  border: none;
  background: transparent;
  cursor: pointer;
}

.setting-toggle:disabled {
  opacity: 0.6;
  cursor: not-allowed;
}

.setting-toggle-track {
  display: block;
  width: 2.4rem;
  height: 1.35rem;
  border-radius: 999px;
  background: var(--modio-surface-raised);
  border: 1px solid var(--modio-border);
  position: relative;
  transition: background 0.2s ease, border-color 0.2s ease;
}

.setting-toggle-track.on {
  background: rgba(var(--modio-accent-rgb), 0.35);
  border-color: rgba(var(--modio-accent-rgb), 0.55);
}

.setting-toggle-thumb {
  position: absolute;
  top: 50%;
  left: 0.15rem;
  width: 1rem;
  height: 1rem;
  border-radius: 50%;
  background: var(--modio-text-muted);
  transform: translateY(-50%);
  transition: transform 0.2s ease, background 0.2s ease;
}

.setting-toggle-track.on .setting-toggle-thumb {
  transform: translate(1.05rem, -50%);
  background: var(--modio-accent);
}

.spinner {
  width: 0.9rem;
  height: 0.9rem;
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

@media (max-width: 640px) {
  .failed-item {
    flex-direction: column;
    align-items: stretch;
  }

  .failed-actions {
    justify-content: space-between;
    flex-wrap: wrap;
  }
}
</style>
