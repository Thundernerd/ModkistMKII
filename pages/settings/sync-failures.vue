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

onMounted(refreshFailedSyncMods);
</script>

<template>
  <div class="page">
    <header class="page-header">
      <NuxtLink to="/settings" class="back-link">← Settings</NuxtLink>
      <h1>Failed sync mods</h1>
      <p class="hint page-desc">
        Subscribed mods that could not be installed during sync. Ignored mods are
        skipped on future syncs.
      </p>
    </header>

    <p v-if="error" class="error feedback">{{ error }}</p>

    <div v-if="loading" class="state">
      <span class="spinner" aria-hidden="true" />
      Loading failed mods…
    </div>

    <p v-else-if="mods.length === 0" class="hint empty-state">
      No failed subscription sync mods.
    </p>

    <ul v-else class="failed-list">
      <li v-for="entry in mods" :key="entry.modId" class="failed-item panel">
        <div class="failed-main">
          <span class="failed-id">Mod ID {{ entry.modId }}</span>
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
              :aria-label="`Ignore mod ${entry.modId}`"
              :disabled="ignoringIds.has(entry.modId)"
              @click="handleIgnoreToggle(entry.modId, !entry.ignored)"
            >
              <span class="setting-toggle-track" :class="{ on: entry.ignored }">
                <span class="setting-toggle-thumb" />
              </span>
            </button>
          </label>

          <button
            type="button"
            class="btn-secondary unsubscribe-btn"
            :disabled="unsubscribingIds.has(entry.modId)"
            @click="handleUnsubscribe(entry.modId)"
          >
            <span v-if="unsubscribingIds.has(entry.modId)" class="spinner" aria-hidden="true" />
            {{ unsubscribingIds.has(entry.modId) ? "Unsubscribing…" : "Unsubscribe" }}
          </button>
        </div>
      </li>
    </ul>
  </div>
</template>

<style scoped>
.page {
  max-width: 40rem;
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
  font-variant-numeric: tabular-nums;
}

.item-error {
  margin: 0.35rem 0 0;
  font-size: 0.82rem;
  color: var(--modio-danger);
}

.failed-actions {
  display: flex;
  align-items: center;
  gap: 0.85rem;
  flex-shrink: 0;
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

.unsubscribe-btn {
  display: inline-flex;
  align-items: center;
  gap: 0.45rem;
  white-space: nowrap;
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
  }
}
</style>
