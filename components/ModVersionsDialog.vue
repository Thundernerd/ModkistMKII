<script setup lang="ts">
import type { ModFileEntry } from "~/composables/useModDetail";
import { formatFileSize, formatRelativeShort } from "~/utils/formatRelative";

const props = defineProps<{
  open: boolean;
  modId: number;
  modName: string;
  installedFileId: number | null;
  installBlocked: boolean;
  gameRunning: boolean;
}>();

const emit = defineEmits<{
  close: [];
  install: [fileId: number, versionLabel: string];
}>();

const { fetchModFiles } = useModDetail();

const files = ref<ModFileEntry[]>([]);
const latestFileId = ref<number | null>(null);
const loading = ref(false);
const error = ref("");
const expandedChangelogId = ref<number | null>(null);

watch(
  () => props.open,
  (isOpen) => {
    if (!isOpen) {
      files.value = [];
      latestFileId.value = null;
      error.value = "";
      expandedChangelogId.value = null;
      return;
    }

    void loadFiles();
  },
);

watch(
  () => props.modId,
  () => {
    if (props.open) {
      void loadFiles();
    }
  },
);

async function loadFiles() {
  loading.value = true;
  error.value = "";
  try {
    const result = await fetchModFiles(props.modId);
    files.value = result.files;
    latestFileId.value = result.latestFileId;
  } catch (err) {
    error.value = err instanceof Error ? err.message : String(err);
    files.value = [];
    latestFileId.value = null;
  } finally {
    loading.value = false;
  }
}

function versionLabel(file: ModFileEntry) {
  return file.version.trim() || file.filename;
}

function canInstallFile(file: ModFileEntry) {
  if (!file.downloadable) {
    return false;
  }
  if (props.installBlocked || props.gameRunning) {
    return false;
  }
  return props.installedFileId !== file.id;
}

function installLabel(file: ModFileEntry) {
  if (props.installedFileId === file.id) {
    return "Installed";
  }
  if (!file.downloadable || props.installBlocked || props.gameRunning) {
    return "Unavailable";
  }
  return "Install";
}

function toggleChangelog(fileId: number) {
  expandedChangelogId.value =
    expandedChangelogId.value === fileId ? null : fileId;
}

function handleKeydown(event: KeyboardEvent) {
  if (event.key === "Escape") {
    emit("close");
  }
}

onMounted(() => {
  document.addEventListener("keydown", handleKeydown);
});

onBeforeUnmount(() => {
  document.removeEventListener("keydown", handleKeydown);
});
</script>

<template>
  <div v-if="open" class="versions-backdrop" @click.self="emit('close')">
    <div
      class="versions-dialog panel"
      role="dialog"
      aria-modal="true"
      :aria-label="`Versions of ${modName}`"
    >
      <div class="versions-header">
        <div>
          <h2 class="versions-title">Versions</h2>
          <p class="hint versions-subtitle">{{ modName }}</p>
        </div>
        <button
          type="button"
          class="versions-close"
          aria-label="Close"
          @click="emit('close')"
        >
          ×
        </button>
      </div>

      <p v-if="loading" class="versions-state">Loading versions…</p>
      <p v-else-if="error" class="error versions-state">{{ error }}</p>
      <p v-else-if="!files.length" class="hint versions-state">
        No downloadable versions found.
      </p>

      <ul v-else class="versions-list">
        <li v-for="file in files" :key="file.id" class="versions-item">
          <div class="versions-item-main">
            <div class="versions-item-heading">
              <span class="versions-item-label">{{ versionLabel(file) }}</span>
              <span v-if="latestFileId === file.id" class="versions-badge">
                Latest
              </span>
              <span
                v-if="installedFileId === file.id"
                class="versions-badge versions-badge-installed"
              >
                Installed
              </span>
            </div>
            <p class="versions-item-meta">
              <span v-if="file.dateAdded">
                {{ formatRelativeShort(file.dateAdded) }}
              </span>
              <span v-if="file.filesize"> · {{ formatFileSize(file.filesize) }}</span>
            </p>
            <button
              v-if="file.changelog.trim()"
              type="button"
              class="versions-changelog-toggle"
              @click="toggleChangelog(file.id)"
            >
              {{
                expandedChangelogId === file.id
                  ? "Hide changelog"
                  : "Show changelog"
              }}
            </button>
            <p
              v-if="expandedChangelogId === file.id && file.changelog.trim()"
              class="versions-changelog"
            >
              {{ file.changelog }}
            </p>
          </div>

          <button
            type="button"
            class="versions-install-btn"
            :class="{
              'versions-install-btn-installed': installedFileId === file.id,
            }"
            :disabled="!canInstallFile(file)"
            @click="emit('install', file.id, versionLabel(file))"
          >
            {{ installLabel(file) }}
          </button>
        </li>
      </ul>
    </div>
  </div>
</template>

<style scoped>
.versions-backdrop {
  position: fixed;
  inset: 0;
  z-index: 100;
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 1rem;
  background: rgba(0, 0, 0, 0.55);
}

.versions-dialog {
  width: min(100%, 32rem);
  max-height: min(80vh, 36rem);
  display: flex;
  flex-direction: column;
  padding: 1.25rem;
  border-radius: var(--modio-radius);
  background: var(--modio-surface);
  border: 1px solid var(--modio-border);
  box-shadow: var(--modio-shadow);
}

.versions-header {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 1rem;
  margin-bottom: 1rem;
}

.versions-title {
  margin: 0 0 0.2rem;
  font-size: 1.1rem;
  font-weight: 600;
}

.versions-subtitle {
  margin: 0;
}

.versions-close {
  flex-shrink: 0;
  width: 2rem;
  height: 2rem;
  padding: 0;
  border: 1px solid var(--modio-border);
  border-radius: var(--modio-radius-sm);
  background: transparent;
  color: var(--modio-text-muted);
  font-size: 1.25rem;
  line-height: 1;
  cursor: pointer;
}

.versions-close:hover {
  background: var(--modio-surface-hover);
  border-color: var(--modio-border);
  color: var(--modio-text);
}

.versions-state {
  margin: 0;
}

.versions-list {
  list-style: none;
  margin: 0;
  padding: 0;
  overflow-y: auto;
  display: flex;
  flex-direction: column;
  gap: 0.65rem;
}

.versions-item {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 0.75rem;
  padding: 0.8rem 0.85rem;
  border: 1px solid var(--modio-border);
  border-radius: var(--modio-radius-sm);
}

.versions-item-main {
  min-width: 0;
}

.versions-item-heading {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 0.4rem;
  margin-bottom: 0.2rem;
}

.versions-item-label {
  font-weight: 600;
}

.versions-badge {
  display: inline-block;
  padding: 0.1rem 0.45rem;
  border-radius: 999px;
  font-size: 0.72rem;
  font-weight: 600;
  background: rgba(var(--modio-accent-rgb), 0.14);
  color: var(--modio-accent);
}

.versions-badge-installed {
  background: rgba(74, 222, 128, 0.12);
  color: var(--modio-success);
}

.versions-item-meta {
  margin: 0;
  font-size: 0.82rem;
  color: var(--modio-text-muted);
}

.versions-changelog-toggle {
  margin-top: 0.35rem;
  padding: 0;
  border: none;
  background: transparent;
  color: var(--modio-accent);
  font-size: 0.8rem;
  font-weight: 500;
  cursor: pointer;
}

.versions-changelog-toggle:hover {
  color: var(--modio-accent-hover);
  background: transparent;
  border-color: transparent;
}

.versions-changelog {
  margin: 0.45rem 0 0;
  font-size: 0.82rem;
  color: var(--modio-text-muted);
  white-space: pre-wrap;
}

.versions-install-btn {
  flex-shrink: 0;
  padding: 0.4rem 0.7rem;
  font-size: 0.8rem;
  background: rgba(var(--modio-accent-rgb), 0.16);
  border-color: rgba(var(--modio-accent-rgb), 0.55);
  color: var(--modio-accent);
}

.versions-install-btn:hover:not(:disabled) {
  background: rgba(var(--modio-accent-rgb), 0.24);
}

.versions-install-btn-installed,
.versions-install-btn:disabled {
  background: var(--modio-surface-raised);
  border-color: var(--modio-border);
  color: var(--modio-text-muted);
  opacity: 0.85;
  cursor: not-allowed;
}
</style>
