<script setup lang="ts">
import { confirm, open } from "@tauri-apps/plugin-dialog";
import type { SideloadedEntry } from "~/composables/useSideload";

definePageMeta({ layout: "app" });

const {
  entries,
  loading,
  adding,
  error,
  refreshSideloaded,
  addSideloaded,
  removeSideloaded,
  isRemoving,
} = useSideload();
const { gameRunning, gameRunningMessage } = useGameProcess();

const pageError = ref("");

async function loadSideloaded() {
  pageError.value = "";
  try {
    await refreshSideloaded();
  } catch (err) {
    pageError.value = err instanceof Error ? err.message : String(err);
  }
}

async function browseForMod() {
  pageError.value = "";
  error.value = "";

  const selected = await open({
    multiple: false,
    title: "Select a mod file",
    filters: [
      { name: "Mod files", extensions: ["dll", "zip"] },
      { name: "DLL files", extensions: ["dll"] },
      { name: "Zip archives", extensions: ["zip"] },
    ],
  });

  if (typeof selected !== "string") {
    return;
  }

  try {
    await addSideloaded(selected);
  } catch (err) {
    pageError.value = err instanceof Error ? err.message : String(err);
  }
}

async function handleRemove(entry: SideloadedEntry) {
  const confirmed = await confirm(
    `Remove "${entry.name}" from BepInEx/plugins/Sideloaded?`,
    { title: "Remove sideloaded mod?", kind: "warning" },
  );
  if (!confirmed) return;

  pageError.value = "";
  try {
    await removeSideloaded(entry.id);
  } catch (err) {
    pageError.value = err instanceof Error ? err.message : String(err);
  }
}

function sourceTypeLabel(sourceType: SideloadedEntry["sourceType"]) {
  return sourceType === "dll" ? "DLL" : "Archive";
}

function formatAddedAt(addedAt?: string) {
  if (!addedAt) return null;
  const date = new Date(addedAt);
  if (Number.isNaN(date.getTime())) return null;
  return date.toLocaleString();
}

const actionsDisabled = computed(
  () => loading.value || adding.value || gameRunning.value,
);

onMounted(loadSideloaded);
</script>

<template>
  <div class="sideload-page">
    <header class="page-header">
      <h1>Sideload</h1>
      <p class="page-subtitle">
        Add your own mods from a DLL or zip archive. Sideloaded mods are global
        and are not tied to profiles, updates, or mod.io subscriptions.
      </p>
    </header>

    <section class="panel">
      <h2 class="panel-title">Add mod</h2>
      <p class="hint panel-desc">
        Files are installed into
        <code>BepInEx/plugins/Sideloaded</code>, each in its own subfolder.
        Close Zeepkist before adding or removing sideloaded mods.
      </p>

      <p v-if="gameRunning" class="hint install-hint">
        {{
          gameRunningMessage ??
          "Zeepkist is running. Close the game before adding or removing sideloaded mods."
        }}
      </p>

      <div class="action-row">
        <button
          type="button"
          :disabled="actionsDisabled"
          @click="browseForMod"
        >
          <span v-if="adding" class="spinner" aria-hidden="true" />
          {{ adding ? "Adding mod…" : "Choose file…" }}
        </button>
      </div>
    </section>

    <p v-if="pageError || error" class="error">
      {{ pageError || error }}
    </p>

    <section class="panel">
      <h2 class="panel-title">Sideloaded mods</h2>

      <div v-if="loading" class="state">
        <span class="spinner" aria-hidden="true" />
        Loading sideloaded mods…
      </div>

      <p v-else-if="entries.length === 0" class="hint empty-state">
        No sideloaded mods yet. Use Choose file to add a .dll or .zip mod.
      </p>

      <ul v-else class="sideload-list">
        <li v-for="entry in entries" :key="entry.id">
          <article class="sideload-card">
            <div class="sideload-info">
              <div class="sideload-title-row">
                <h2>{{ entry.name }}</h2>
                <span class="kind-badge">{{ sourceTypeLabel(entry.sourceType) }}</span>
              </div>
              <p v-if="formatAddedAt(entry.addedAt)" class="sideload-meta">
                Added {{ formatAddedAt(entry.addedAt) }}
              </p>
            </div>

            <button
              type="button"
              class="btn-danger"
              :disabled="actionsDisabled || isRemoving(entry.id)"
              @click="handleRemove(entry)"
            >
              {{ isRemoving(entry.id) ? "Removing…" : "Remove" }}
            </button>
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

.panel {
  margin-bottom: 1rem;
}

.panel-title {
  margin: 0 0 0.5rem;
  font-size: 1rem;
  font-weight: 600;
}

.panel-desc {
  margin: 0 0 1rem;
}

.install-hint,
.empty-state {
  margin-bottom: 1rem;
  padding: 1rem 1.1rem;
  border-radius: var(--modio-radius);
  border: 1px dashed var(--modio-border);
  background: var(--modio-surface);
}

.action-row {
  display: flex;
  gap: 0.75rem;
}

.state {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 0.75rem;
  padding: 2rem 1rem;
  color: var(--modio-text-muted);
}

.spinner {
  display: inline-block;
  width: 1rem;
  height: 1rem;
  margin-right: 0.35rem;
  border: 2px solid var(--modio-border);
  border-top-color: var(--modio-accent);
  border-radius: 50%;
  animation: spin 0.7s linear infinite;
  vertical-align: -0.15rem;
}

@keyframes spin {
  to {
    transform: rotate(360deg);
  }
}

.sideload-list {
  list-style: none;
  margin: 0;
  padding: 0;
  display: flex;
  flex-direction: column;
  gap: 0.85rem;
}

.sideload-card {
  display: grid;
  grid-template-columns: minmax(0, 1fr) auto;
  gap: 1rem;
  align-items: center;
  padding: 0.9rem 1rem;
  border-radius: var(--modio-radius);
  border: 1px solid var(--modio-border);
  background: var(--modio-surface);
}

.sideload-title-row {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 0.45rem;
}

.sideload-info h2 {
  margin: 0;
  font-size: 1rem;
  font-weight: 600;
}

.sideload-meta {
  margin: 0.4rem 0 0;
  font-size: 0.78rem;
  color: var(--modio-text-muted);
}

.kind-badge {
  padding: 0.15rem 0.5rem;
  border-radius: 999px;
  font-size: 0.72rem;
  font-weight: 600;
  background: var(--modio-surface-raised);
  color: var(--modio-text-muted);
}

@media (max-width: 760px) {
  .sideload-card {
    grid-template-columns: 1fr;
  }
}
</style>
