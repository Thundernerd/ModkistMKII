<script setup lang="ts">
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { confirm, open } from "@tauri-apps/plugin-dialog";
import type { UnlistenFn } from "@tauri-apps/api/event";
import type { SideloadedEntry, SideloadTargetKind } from "~/composables/useSideload";

definePageMeta({ layout: "app" });

const ACCEPTED_EXTENSIONS = new Set(["dll", "zeeplevel", "zip"]);

const {
  entries,
  loading,
  adding,
  error,
  refreshSideloaded,
  addSideloaded,
  removeSideloaded,
  openSideloadedFolder,
  renameSideloaded,
  isRemoving,
  isOpening,
  isRenaming,
} = useSideload();
const { gameRunning, gameRunningMessage } = useGameProcess();

const pageError = ref("");
const targetChoiceOpen = ref(false);
const pendingSourcePaths = ref<string[]>([]);
const pendingFolderName = ref("");
const pendingUseSymlinks = ref(false);
const linking = ref(false);
const renameOpen = ref(false);
const renameEntry = ref<SideloadedEntry | null>(null);
const dropzoneRef = ref<HTMLElement | null>(null);
const dragActive = ref(false);
let unlistenDragDrop: UnlistenFn | undefined;

async function loadSideloaded() {
  pageError.value = "";
  try {
    await refreshSideloaded();
  } catch (err) {
    pageError.value = err instanceof Error ? err.message : String(err);
  }
}

function extensionOf(path: string) {
  const base = path.split(/[\\/]/).pop() ?? path;
  const dot = base.lastIndexOf(".");
  if (dot <= 0 || dot === base.length - 1) return "";
  return base.slice(dot + 1).toLowerCase();
}

function filterAcceptedPaths(paths: string[]) {
  return paths.filter((path) => ACCEPTED_EXTENSIONS.has(extensionOf(path)));
}

async function browseForMod() {
  pageError.value = "";
  error.value = "";

  const selected = await open({
    multiple: true,
    title: "Select mod files",
    filters: [
      { name: "Mod files", extensions: ["dll", "zeeplevel", "zip"] },
      { name: "DLL files", extensions: ["dll"] },
      { name: "Blueprint files", extensions: ["zeeplevel"] },
      { name: "Zip archives", extensions: ["zip"] },
    ],
  });

  if (selected == null) {
    return;
  }

  const paths = Array.isArray(selected) ? selected : [selected];
  if (paths.length === 0) {
    return;
  }

  await handleAddSideloaded(paths);
}

async function browseForLink() {
  pageError.value = "";
  error.value = "";

  const selected = await open({
    multiple: true,
    title: "Select files to link",
  });

  if (selected == null) {
    return;
  }

  const paths = Array.isArray(selected) ? selected : [selected];
  if (paths.length === 0) {
    return;
  }

  await handleAddSideloaded(paths, undefined, true);
}

async function handleAddSideloaded(
  sourcePaths: string[],
  targetKind?: SideloadTargetKind,
  useSymlinks = false,
) {
  pageError.value = "";
  error.value = "";

  if (useSymlinks) {
    linking.value = true;
  }

  try {
    const result = await addSideloaded(sourcePaths, targetKind, useSymlinks);

    if (result.status === "needsTargetChoice") {
      pendingSourcePaths.value = result.sourcePaths ?? [];
      pendingFolderName.value = result.folderName ?? "";
      pendingUseSymlinks.value = useSymlinks;
      targetChoiceOpen.value = true;
      return;
    }
  } catch (err) {
    pageError.value = err instanceof Error ? err.message : String(err);
  } finally {
    linking.value = false;
  }
}

async function handleTargetChoice(targetKind: SideloadTargetKind) {
  targetChoiceOpen.value = false;
  const sourcePaths = pendingSourcePaths.value;
  const useSymlinks = pendingUseSymlinks.value;
  pendingSourcePaths.value = [];
  pendingFolderName.value = "";
  pendingUseSymlinks.value = false;

  if (!sourcePaths || sourcePaths.length === 0) {
    return;
  }

  await handleAddSideloaded(sourcePaths, targetKind, useSymlinks);
}

function closeTargetChoice() {
  targetChoiceOpen.value = false;
  pendingSourcePaths.value = [];
  pendingFolderName.value = "";
  pendingUseSymlinks.value = false;
}

async function handleRemove(entry: SideloadedEntry) {
  const confirmed = await confirm(
    `Remove "${entry.name}" from BepInEx/plugins/Sideloaded/${entry.id}?`,
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

async function handleOpenFolder(entry: SideloadedEntry) {
  pageError.value = "";
  try {
    await openSideloadedFolder(entry.id);
  } catch (err) {
    pageError.value = err instanceof Error ? err.message : String(err);
  }
}

function openRename(entry: SideloadedEntry) {
  pageError.value = "";
  renameEntry.value = entry;
  renameOpen.value = true;
}

function closeRename() {
  renameOpen.value = false;
  renameEntry.value = null;
}

async function handleRename(newName: string) {
  const entry = renameEntry.value;
  if (!entry) return;

  pageError.value = "";
  try {
    await renameSideloaded(entry.id, newName);
    closeRename();
  } catch (err) {
    pageError.value = err instanceof Error ? err.message : String(err);
  }
}

function targetKindLabel(targetKind: SideloadedEntry["targetKind"]) {
  return targetKind === "plugins" ? "Plugin" : "Blueprint";
}

function formatAddedAt(addedAt?: string) {
  if (!addedAt) return null;
  const date = new Date(addedAt);
  if (Number.isNaN(date.getTime())) return null;
  return date.toLocaleString();
}

const actionsDisabled = computed(
  () => loading.value || adding.value || linking.value || gameRunning.value,
);

function isPointInDropzone(x: number, y: number) {
  const el = dropzoneRef.value;
  if (!el) return false;
  const rect = el.getBoundingClientRect();
  return x >= rect.left && x <= rect.right && y >= rect.top && y <= rect.bottom;
}

async function isOverDropzone(position: { x: number; y: number; toLogical?: (scaleFactor: number) => { x: number; y: number } }) {
  const scaleFactor = await getCurrentWindow().scaleFactor();
  const logical = position.toLogical
    ? position.toLogical(scaleFactor)
    : { x: position.x / scaleFactor, y: position.y / scaleFactor };
  return isPointInDropzone(logical.x, logical.y);
}

onMounted(async () => {
  await loadSideloaded();

  try {
    unlistenDragDrop = await getCurrentWebview().onDragDropEvent(async (event) => {
      const payload = event.payload;

      if (payload.type === "leave") {
        dragActive.value = false;
        return;
      }

      if (payload.type === "enter" || payload.type === "over") {
        if (actionsDisabled.value) {
          dragActive.value = false;
          return;
        }
        dragActive.value = await isOverDropzone(payload.position);
        return;
      }

      if (payload.type === "drop") {
        const wasActive = dragActive.value;
        dragActive.value = false;

        if (actionsDisabled.value) {
          return;
        }

        const overZone =
          wasActive || (await isOverDropzone(payload.position));
        if (!overZone) {
          return;
        }

        const accepted = filterAcceptedPaths(payload.paths);
        if (accepted.length === 0) {
          pageError.value =
            "Drop .dll, .zeeplevel, or .zip files to sideload.";
          return;
        }

        await handleAddSideloaded(accepted);
      }
    });
  } catch {
    // Not running inside Tauri (e.g. browser preview).
  }
});

onUnmounted(() => {
  unlistenDragDrop?.();
});
</script>

<template>
  <div class="sideload-page">
    <header class="page-header">
      <h1>Sideload</h1>
      <p class="page-subtitle">
        Add your own mods from a DLL, blueprint file, or zip archive.
        Sideloaded mods are global and are not tied to profiles, updates, or
        mod.io subscriptions.
      </p>
    </header>

    <section class="panel">
      <h2 class="panel-title">Add mod</h2>
      <p class="hint panel-desc">
        DLLs go into <code>Sideloaded/Plugins</code>. Blueprint files go into
        <code>Sideloaded/Blueprints</code>. Each file uses its own subfolder.
        Zip archives are classified automatically. If an archive contains both
        types, you can choose the target. Multiple loose files install as one
        entry. Link files creates symlinks in Sideloaded for the files that you
        select. Your originals stay in place. This is useful for mod development.
        If a file is not a mod file, choose Plugins or Blueprints. Zip archives
        must still be copied. If Zeepkist is open, close it before you add or
        remove sideloaded mods.
      </p>

      <p v-if="gameRunning" class="hint install-hint">
        {{
          gameRunningMessage ??
          "Zeepkist is running. Close the game before adding or removing sideloaded mods."
        }}
      </p>

      <div
        ref="dropzoneRef"
        class="dropzone"
        :class="{
          'dropzone-active': dragActive && !actionsDisabled,
          'dropzone-disabled': actionsDisabled,
        }"
      >
        <p class="dropzone-label">
          Drop .dll, .zeeplevel, or .zip files here, or choose files.
        </p>
        <div class="action-row">
          <button
            type="button"
            :disabled="actionsDisabled"
            @click="browseForMod"
          >
            <span v-if="adding && !linking" class="spinner" aria-hidden="true" />
            {{ adding && !linking ? "Adding mod…" : "Choose files…" }}
          </button>
          <button
            type="button"
            class="btn-secondary"
            :disabled="actionsDisabled"
            @click="browseForLink"
          >
            <span v-if="linking" class="spinner" aria-hidden="true" />
            {{ linking ? "Linking files…" : "Link files…" }}
          </button>
        </div>
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
        No sideloaded mods yet. Drop or choose .dll, .zeeplevel, or .zip files
        to add a mod.
      </p>

      <ul v-else class="sideload-list">
        <li v-for="entry in entries" :key="entry.id">
          <article class="sideload-card">
            <div class="sideload-info">
              <div class="sideload-title-row">
                <h2>{{ entry.name }}</h2>
                <span class="ui-pill kind-badge"><span class="ui-pill-text">{{ targetKindLabel(entry.targetKind) }}</span></span>
                <span v-if="entry.linked" class="ui-pill kind-badge linked-badge"><span class="ui-pill-text">Linked</span></span>
              </div>
              <p class="sideload-meta">
                {{ entry.id }}
              </p>
              <p v-if="formatAddedAt(entry.addedAt)" class="sideload-meta">
                Added {{ formatAddedAt(entry.addedAt) }}
              </p>
            </div>

            <div class="sideload-actions">
              <button
                type="button"
                class="btn-secondary"
                :disabled="loading || adding || linking || isOpening(entry.id)"
                @click="handleOpenFolder(entry)"
              >
                {{ isOpening(entry.id) ? "Opening…" : "Open folder" }}
              </button>
              <button
                type="button"
                class="btn-secondary"
                :disabled="actionsDisabled || isRenaming(entry.id)"
                @click="openRename(entry)"
              >
                Rename
              </button>
              <button
                type="button"
                class="btn-danger"
                :disabled="actionsDisabled || isRemoving(entry.id)"
                @click="handleRemove(entry)"
              >
                {{ isRemoving(entry.id) ? "Removing…" : "Remove" }}
              </button>
            </div>
          </article>
        </li>
      </ul>
    </section>

    <SideloadTargetDialog
      :open="targetChoiceOpen"
      :folder-name="pendingFolderName"
      @close="closeTargetChoice"
      @select="handleTargetChoice"
    />

    <SideloadRenameDialog
      :open="renameOpen"
      :current-name="renameEntry?.name ?? ''"
      :busy="renameEntry ? isRenaming(renameEntry.id) : false"
      @close="closeRename"
      @rename="handleRename"
    />
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

.dropzone {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 0.85rem;
  padding: 1.35rem 1.1rem;
  border-radius: var(--modio-radius);
  border: 1px dashed var(--modio-border);
  background: var(--modio-surface);
  transition:
    border-color 0.15s ease,
    background-color 0.15s ease,
    box-shadow 0.15s ease;
}

.dropzone-active {
  border-color: var(--modio-accent);
  background: var(--modio-surface-raised);
  box-shadow: 0 0 0 2px rgba(var(--modio-accent-rgb), 0.2);
}

.dropzone-disabled {
  opacity: 0.65;
}

.dropzone-label {
  margin: 0;
  text-align: center;
  color: var(--modio-text-muted);
  font-size: 0.9rem;
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

.sideload-actions {
  display: flex;
  flex-wrap: wrap;
  gap: 0.5rem;
  justify-content: flex-end;
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
  background: var(--modio-surface-raised);
  color: var(--modio-text-muted);
}

.linked-badge {
  color: var(--modio-accent);
}

@media (max-width: 760px) {
  .sideload-card {
    grid-template-columns: 1fr;
  }
}
</style>
