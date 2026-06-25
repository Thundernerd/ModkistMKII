<script setup lang="ts">
import { confirm } from "@tauri-apps/plugin-dialog";

definePageMeta({ layout: "app" });

const {
  installedMods,
  installEnvironmentError,
  refreshInstalled,
  installMod,
  uninstallMod,
  getUiStatus,
  getCanUninstall,
  getInstallError,
  isUninstalling,
  gameRunning,
  gameRunningMessage,
} = useModInstall();

const loading = ref(true);
const pageError = ref("");

async function loadInstalled() {
  loading.value = true;
  pageError.value = "";
  try {
    await refreshInstalled();
  } catch (error) {
    pageError.value = error instanceof Error ? error.message : String(error);
  } finally {
    loading.value = false;
  }
}

async function handleInstall(modId: number) {
  await installMod(modId);
}

async function handleUninstall(modId: number, name: string) {
  const confirmed = await confirm(
    `Remove "${name}" from your game folder?`,
    { title: "Uninstall mod?", kind: "warning" },
  );
  if (!confirmed) return;
  await uninstallMod(modId);
}

function kindLabel(kind: "plugin" | "blueprint") {
  return kind === "plugin" ? "Plugin" : "Blueprint";
}

onMounted(loadInstalled);
</script>

<template>
  <div class="installed-page">
    <header class="page-header">
      <h1>Installed</h1>
      <p class="page-subtitle">
        Mods extracted into your BepInEx plugins folder.
      </p>
    </header>

    <p v-if="installEnvironmentError" class="hint install-hint">
      {{ installEnvironmentError }}
      <NuxtLink to="/settings">Check Settings</NuxtLink>
    </p>

    <p v-else-if="gameRunning" class="hint install-hint">
      {{ gameRunningMessage ?? "Zeepkist is running. Close the game before removing mods." }}
    </p>

    <p v-if="pageError" class="error">{{ pageError }}</p>

    <div v-if="loading" class="state">
      <span class="spinner" aria-hidden="true" />
      Loading installed mods…
    </div>

    <div
      v-else-if="!installEnvironmentError && installedMods.length === 0"
      class="hint empty-state"
    >
      No mods installed yet. Browse mods and use Install to add them.
      <NuxtLink to="/home">Browse mods</NuxtLink>
    </div>

    <ul v-else-if="installedMods.length" class="installed-list">
      <li v-for="mod in installedMods" :key="`${mod.modId}-${mod.fileId}`">
        <article class="installed-card">
          <NuxtLink :to="`/mods/${mod.modId}`" class="installed-card-link">
            <div class="installed-thumb">
              <img
                v-if="mod.logoUrl"
                :src="mod.logoUrl"
                :alt="`${mod.name} logo`"
                loading="lazy"
              />
              <div v-else class="installed-thumb-fallback" />
            </div>

            <div class="installed-info">
              <div class="installed-title-row">
                <h2>{{ mod.name }}</h2>
                <span class="kind-badge">{{ kindLabel(mod.kind) }}</span>
                <span v-if="mod.updateAvailable" class="update-badge">Update</span>
              </div>
              <p class="installed-summary">{{ mod.summary }}</p>
              <p class="installed-meta">
                File {{ mod.fileId }}
                <span v-if="mod.updateAvailable && mod.latestFileId">
                  · Latest {{ mod.latestFileId }}
                </span>
              </p>
            </div>
          </NuxtLink>

          <div class="installed-actions">
            <ModInstallButton
              :mod-id="mod.modId"
              :status="getUiStatus(mod.modId)"
              :can-uninstall="getCanUninstall(mod.modId)"
              :is-uninstalling="isUninstalling(mod.modId)"
              :error="getInstallError(mod.modId)"
              @install="handleInstall(mod.modId)"
              @uninstall="handleUninstall(mod.modId, mod.name)"
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

.installed-list {
  list-style: none;
  margin: 0;
  padding: 0;
  display: flex;
  flex-direction: column;
  gap: 0.85rem;
}

.installed-card {
  display: grid;
  grid-template-columns: minmax(0, 1fr) auto;
  gap: 1rem;
  align-items: center;
  padding: 0.9rem 1rem;
  border-radius: var(--modio-radius);
  border: 1px solid var(--modio-border);
  background: var(--modio-surface);
}

.installed-card-link {
  display: grid;
  grid-template-columns: 6.5rem minmax(0, 1fr);
  gap: 0.9rem;
  align-items: center;
  color: inherit;
  text-decoration: none;
}

.installed-card-link:hover {
  color: inherit;
}

.installed-thumb {
  aspect-ratio: 16 / 9;
  border-radius: var(--modio-radius-sm);
  overflow: hidden;
  background: var(--modio-surface-raised);
}

.installed-thumb img {
  width: 100%;
  height: 100%;
  object-fit: cover;
  display: block;
}

.installed-thumb-fallback {
  width: 100%;
  height: 100%;
  background: linear-gradient(
    135deg,
    var(--modio-surface-raised),
    var(--modio-surface-hover)
  );
}

.installed-title-row {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 0.45rem;
}

.installed-info h2 {
  margin: 0;
  font-size: 1rem;
  font-weight: 600;
}

.installed-summary {
  margin: 0.35rem 0 0;
  font-size: 0.85rem;
  color: var(--modio-text-subtle);
  display: -webkit-box;
  -webkit-line-clamp: 2;
  -webkit-box-orient: vertical;
  overflow: hidden;
}

.installed-meta {
  margin: 0.4rem 0 0;
  font-size: 0.78rem;
  color: var(--modio-text-muted);
}

.kind-badge,
.update-badge {
  padding: 0.15rem 0.5rem;
  border-radius: 999px;
  font-size: 0.72rem;
  font-weight: 600;
}

.kind-badge {
  background: var(--modio-surface-raised);
  color: var(--modio-text-muted);
}

.update-badge {
  background: rgba(7, 193, 216, 0.14);
  color: var(--modio-accent);
}

.installed-actions {
  min-width: 8.5rem;
}

@media (max-width: 760px) {
  .installed-card {
    grid-template-columns: 1fr;
  }

  .installed-actions {
    min-width: 0;
  }
}
</style>
