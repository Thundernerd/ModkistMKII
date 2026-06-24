<script setup lang="ts">
import { confirm } from "@tauri-apps/plugin-dialog";
import type { DependencySort, ModDependency } from "~/composables/useModDetail";
import {
  formatFileSize,
  formatRelativeAgo,
  formatRelativeShort,
} from "~/utils/formatRelative";

definePageMeta({ layout: "app" });

const route = useRoute();
const {
  mod,
  dependencies,
  loading,
  dependenciesLoading,
  error,
  dependenciesError,
  fetchMod,
  fetchDependencies,
} = useModDetail();

const {
  refreshInstalled,
  refreshInstallState,
  installMod,
  uninstallMod,
  getUiStatus,
  getCanUninstall,
  getInstallError,
  isUninstalling,
  installEnvironmentError,
} = useModInstall();

const modId = computed(() => Number(route.params.id));
const isValidId = computed(() => Number.isInteger(modId.value) && modId.value > 0);

type TabId = "description" | "dependencies";
const activeTab = ref<TabId>("description");
const mediaIndex = ref(0);
const dependencySort = ref<DependencySort>("mostPopular");
const copiedId = ref(false);

const mediaImages = computed(() => {
  if (!mod.value) return [];
  return mod.value.mediaImageUrls.length
    ? mod.value.mediaImageUrls
    : [mod.value.heroImageUrl];
});

const sortedDependencies = computed(() => {
  const items = [...dependencies.value];

  switch (dependencySort.value) {
    case "lastUpdated":
      return items.sort(
        (a, b) =>
          new Date(b.dateUpdated).getTime() - new Date(a.dateUpdated).getTime(),
      );
    case "alphabetical":
      return items.sort((a, b) => a.name.localeCompare(b.name));
    case "mostPopular":
    default:
      return items.sort((a, b) => b.downloadsTotal - a.downloadsTotal);
  }
});

watch(
  modId,
  (id) => {
    if (isValidId.value) {
      activeTab.value = "description";
      mediaIndex.value = 0;
      fetchMod(id);
      refreshInstallState(id);
    }
  },
  { immediate: true },
);

watch(
  () => mod.value?.hasDependencies,
  (hasDependencies) => {
    if (hasDependencies && isValidId.value) {
      fetchDependencies(modId.value);
    }
  },
);

watch(
  () => dependencies.value.length,
  () => {
    if (!dependencies.value.length) return;
    for (const dep of dependencies.value) {
      refreshInstallState(dep.id);
    }
  },
);

watch(mediaImages, () => {
  mediaIndex.value = 0;
});

onMounted(() => {
  refreshInstalled();
});

function formatCount(value: number) {
  return value.toLocaleString();
}

function formatLiveDate(iso: string) {
  if (!iso) return "";
  return new Date(iso).toLocaleDateString(undefined, {
    month: "short",
    day: "numeric",
    year: "numeric",
  });
}

function showPreviousImage() {
  if (!mediaImages.value.length) return;
  mediaIndex.value =
    (mediaIndex.value - 1 + mediaImages.value.length) % mediaImages.value.length;
}

function showNextImage() {
  if (!mediaImages.value.length) return;
  mediaIndex.value = (mediaIndex.value + 1) % mediaImages.value.length;
}

function selectTab(tab: TabId) {
  activeTab.value = tab;
}

async function copyModId() {
  if (!mod.value) return;

  try {
    await navigator.clipboard.writeText(String(mod.value.id));
    copiedId.value = true;
    window.setTimeout(() => {
      copiedId.value = false;
    }, 1500);
  } catch {
    copiedId.value = false;
  }
}

async function handleInstall(targetModId = modId.value) {
  await installMod(targetModId);
  await refreshInstalled();
}

async function handleUninstall(targetModId = modId.value, modName?: string) {
  const name = modName ?? mod.value?.name ?? "this mod";
  const confirmed = await confirm(
    `Remove "${name}" from your game folder?`,
    { title: "Uninstall mod?", kind: "warning" },
  );
  if (!confirmed) return;
  await uninstallMod(targetModId);
  await refreshInstalled();
}

function dependencyMeta(dep: ModDependency) {
  const updated = formatRelativeAgo(dep.dateUpdated);
  return updated
    ? `${dep.submittedByUsername} • Updated ${updated}`
    : dep.submittedByUsername;
}
</script>

<template>
  <div class="mod-detail-page">
    <header class="mod-detail-header">
      <NuxtLink to="/home" class="back-link">← Back to mods</NuxtLink>
    </header>

    <div v-if="!isValidId" class="state">
      <p class="error">Invalid mod ID.</p>
      <NuxtLink to="/home" class="back-link">← Back to mods</NuxtLink>
    </div>

    <div v-else-if="loading && !mod" class="state">
      <span class="spinner" aria-hidden="true" />
      Loading mod…
    </div>

    <div v-else-if="error" class="state">
      <p class="error">{{ error }}</p>
      <NuxtLink to="/home" class="back-link">← Back to mods</NuxtLink>
    </div>

    <article v-else-if="mod" class="mod-detail">
      <div class="mod-detail-layout">
        <div class="mod-detail-main">
          <div class="media-viewer">
            <img
              :src="mediaImages[mediaIndex]"
              :alt="`${mod.name} media ${mediaIndex + 1}`"
              class="media-image"
            />

            <button
              v-if="mediaImages.length > 1"
              type="button"
              class="media-nav media-nav-prev"
              aria-label="Previous image"
              @click="showPreviousImage"
            >
              ‹
            </button>
            <button
              v-if="mediaImages.length > 1"
              type="button"
              class="media-nav media-nav-next"
              aria-label="Next image"
              @click="showNextImage"
            >
              ›
            </button>

            <div v-if="mediaImages.length > 1" class="media-dots">
              <button
                v-for="(_, index) in mediaImages"
                :key="index"
                type="button"
                class="media-dot"
                :class="{ active: index === mediaIndex }"
                :aria-label="`Show image ${index + 1}`"
                @click="mediaIndex = index"
              />
            </div>
          </div>

          <section class="tabs-panel">
            <nav class="tabs-nav" aria-label="Mod details">
              <button
                type="button"
                class="tab"
                :class="{ active: activeTab === 'description' }"
                @click="selectTab('description')"
              >
                Description
              </button>
              <button
                v-if="mod.hasDependencies"
                type="button"
                class="tab"
                :class="{ active: activeTab === 'dependencies' }"
                @click="selectTab('dependencies')"
              >
                Dependencies
                <span
                  v-if="dependencies.length || dependenciesLoading"
                  class="tab-badge"
                >
                  {{
                    dependenciesLoading && !dependencies.length
                      ? "…"
                      : dependencies.length
                  }}
                </span>
              </button>
            </nav>

            <div v-if="activeTab === 'description'" class="tab-content">
              <div
                v-if="mod.descriptionHtml"
                class="mod-description"
                v-html="mod.descriptionHtml"
              />
              <p v-else class="tab-empty">
                {{ mod.summary || "No description provided." }}
              </p>
            </div>

            <div v-else class="tab-content">
              <div class="dependencies-toolbar">
                <p class="dependencies-hint">
                  All dependencies need to be installed for this mod to work.
                </p>
                <label class="sort-control">
                  <span class="sort-icon" aria-hidden="true">⇅</span>
                  <select v-model="dependencySort" class="sort-select">
                    <option value="mostPopular">Most popular</option>
                    <option value="lastUpdated">Last updated</option>
                    <option value="alphabetical">Alphabetical</option>
                  </select>
                </label>
              </div>

              <div v-if="dependenciesLoading && !dependencies.length" class="tab-loading">
                <span class="spinner" aria-hidden="true" />
                Loading dependencies…
              </div>

              <p v-else-if="dependenciesError" class="error tab-error">
                {{ dependenciesError }}
              </p>

              <p v-else-if="!sortedDependencies.length" class="tab-empty">
                No dependencies listed.
              </p>

              <ul v-else class="dependency-list">
                <li v-for="dep in sortedDependencies" :key="dep.id">
                  <div class="dependency-card">
                    <NuxtLink :to="`/mods/${dep.id}`" class="dependency-card-link">
                      <div class="dependency-thumb">
                        <img
                          v-if="dep.logoUrl"
                          :src="dep.logoUrl"
                          :alt="`${dep.name} logo`"
                          loading="lazy"
                        />
                        <div v-else class="dependency-thumb-fallback" />
                      </div>

                      <div class="dependency-info">
                        <h3>{{ dep.name }}</h3>
                        <p class="dependency-meta">{{ dependencyMeta(dep) }}</p>
                      </div>
                    </NuxtLink>

                    <div class="dependency-actions">
                      <span
                        v-if="dep.fileSizeBytes"
                        class="dependency-size"
                      >
                        {{ formatFileSize(dep.fileSizeBytes) }}
                      </span>
                      <ModInstallButton
                        :mod-id="dep.id"
                        :status="getUiStatus(dep.id)"
                        :can-uninstall="getCanUninstall(dep.id)"
                        :is-uninstalling="isUninstalling(dep.id)"
                        :error="getInstallError(dep.id)"
                        compact
                        @install="handleInstall(dep.id)"
                        @uninstall="handleUninstall(dep.id, dep.name)"
                      />
                    </div>
                  </div>
                </li>
              </ul>
            </div>
          </section>
        </div>

        <aside class="mod-detail-sidebar">
          <h1 class="sidebar-title">{{ mod.name }}</h1>

          <p v-if="installEnvironmentError" class="sidebar-install-hint">
            {{ installEnvironmentError }}
          </p>

          <ModInstallButton
            :mod-id="mod.id"
            :status="getUiStatus(mod.id)"
            :can-uninstall="getCanUninstall(mod.id)"
            :is-uninstalling="isUninstalling(mod.id)"
            :error="getInstallError(mod.id)"
            @install="handleInstall(mod.id)"
            @uninstall="handleUninstall(mod.id, mod.name)"
          />

          <a
            :href="mod.profileUrl"
            target="_blank"
            rel="noopener noreferrer"
            class="subscribe-button"
          >
            View on mod.io
          </a>

          <section
            v-if="mod.ratingsDisplayText || mod.ratingsPercentagePositive"
            class="sidebar-section ratings-section"
          >
            <div class="ratings-header">
              <span class="ratings-label">{{ mod.ratingsDisplayText }}</span>
              <span
                v-if="mod.ratingsPercentagePositive"
                class="ratings-percent"
              >
                {{ mod.ratingsPercentagePositive }}%
              </span>
            </div>
            <div class="ratings-bar" aria-hidden="true">
              <div
                class="ratings-bar-fill"
                :style="{ width: `${mod.ratingsPercentagePositive}%` }"
              />
            </div>
            <div class="ratings-votes">
              <span class="vote vote-like">
                <span class="vote-icon" aria-hidden="true">👍</span>
                {{ formatCount(mod.ratingsPositive) }}
              </span>
              <span class="vote vote-dislike">
                <span class="vote-icon" aria-hidden="true">👎</span>
                {{ formatCount(mod.ratingsNegative) }}
              </span>
            </div>
          </section>

          <dl class="stats-list">
            <div class="stat-row">
              <dt>Total downloads</dt>
              <dd>{{ formatCount(mod.downloadsTotal) }}</dd>
            </div>
            <div class="stat-row">
              <dt>Today's downloads</dt>
              <dd>{{ formatCount(mod.downloadsToday) }}</dd>
            </div>
            <div class="stat-row">
              <dt>Subscribers</dt>
              <dd>{{ formatCount(mod.subscribersTotal) }}</dd>
            </div>
            <div v-if="mod.dateUpdated" class="stat-row">
              <dt>Last updated</dt>
              <dd>{{ formatRelativeShort(mod.dateUpdated) }}</dd>
            </div>
            <div v-if="mod.dateLive" class="stat-row">
              <dt>Date live</dt>
              <dd>{{ formatLiveDate(mod.dateLive) }}</dd>
            </div>
            <div class="stat-row">
              <dt>ID</dt>
              <dd class="stat-id">
                {{ mod.id }}
                <button
                  type="button"
                  class="copy-id"
                  :aria-label="copiedId ? 'Copied' : 'Copy mod ID'"
                  @click="copyModId"
                >
                  {{ copiedId ? "✓" : "⧉" }}
                </button>
              </dd>
            </div>
          </dl>

          <section v-if="mod.tags.length" class="sidebar-section">
            <h2 class="sidebar-heading">Tags</h2>
            <div class="tag-list">
              <span v-for="tag in mod.tags" :key="tag" class="tag-pill">
                {{ tag }}
              </span>
            </div>
          </section>

          <section class="sidebar-section">
            <h2 class="sidebar-heading">Creators</h2>
            <a
              :href="mod.submittedByProfileUrl"
              target="_blank"
              rel="noopener noreferrer"
              class="creator-row"
            >
              <img
                v-if="mod.submittedByAvatarUrl"
                :src="mod.submittedByAvatarUrl"
                :alt="`${mod.submittedByUsername} avatar`"
                class="creator-avatar"
              />
              <div v-else class="creator-avatar creator-avatar-fallback" />
              <span class="creator-name">{{ mod.submittedByUsername }}</span>
              <span class="creator-link-icon" aria-hidden="true">↗</span>
            </a>
          </section>

          <a
            v-if="mod.homepageUrl"
            :href="mod.homepageUrl"
            target="_blank"
            rel="noopener noreferrer"
            class="homepage-link"
          >
            Homepage ↗
          </a>
        </aside>
      </div>
    </article>
  </div>
</template>

<style scoped>
.mod-detail-page {
  width: 100%;
}

.mod-detail-header {
  margin-bottom: 1.25rem;
}

.back-link {
  font-size: 0.9rem;
  font-weight: 500;
}

.mod-detail-layout {
  display: grid;
  grid-template-columns: minmax(0, 1fr) 20rem;
  gap: 1.5rem;
  align-items: start;
  width: 100%;
}

.mod-detail-main {
  min-width: 0;
}

.media-viewer {
  position: relative;
  aspect-ratio: 16 / 9;
  border-radius: var(--modio-radius);
  overflow: hidden;
  background: var(--modio-surface-raised);
  border: 1px solid var(--modio-border);
}

.media-image {
  width: 100%;
  height: 100%;
  object-fit: cover;
  display: block;
}

.media-nav {
  position: absolute;
  top: 50%;
  transform: translateY(-50%);
  width: 2rem;
  height: 2rem;
  padding: 0;
  border-radius: 999px;
  border: 1px solid var(--modio-border);
  background: rgba(0, 0, 0, 0.55);
  color: var(--modio-text);
  font-size: 1.25rem;
  line-height: 1;
  display: grid;
  place-items: center;
}

.media-nav:hover:not(:disabled) {
  background: rgba(0, 0, 0, 0.75);
  border-color: var(--modio-accent);
}

.media-nav-prev {
  left: 0.75rem;
}

.media-nav-next {
  right: 0.75rem;
}

.media-dots {
  position: absolute;
  left: 0;
  right: 0;
  bottom: 0.65rem;
  display: flex;
  justify-content: center;
  gap: 0.35rem;
}

.media-dot {
  width: 0.45rem;
  height: 0.45rem;
  padding: 0;
  border-radius: 999px;
  border: none;
  background: rgba(255, 255, 255, 0.35);
}

.media-dot.active {
  background: var(--modio-accent);
}

.tabs-panel {
  margin-top: 1.25rem;
}

.tabs-nav {
  display: flex;
  gap: 1.25rem;
  border-bottom: 1px solid var(--modio-border);
}

.tab {
  position: relative;
  padding: 0.65rem 0 0.85rem;
  border: none;
  background: none;
  color: var(--modio-text-muted);
  font-size: 0.95rem;
  font-weight: 600;
  display: inline-flex;
  align-items: center;
  gap: 0.45rem;
}

.tab:hover:not(:disabled) {
  color: var(--modio-text);
  background: none;
}

.tab.active {
  color: var(--modio-text);
}

.tab.active::after {
  content: "";
  position: absolute;
  left: 0;
  right: 0;
  bottom: -1px;
  height: 2px;
  background: var(--modio-accent);
}

.tab-badge {
  min-width: 1.15rem;
  height: 1.15rem;
  padding: 0 0.3rem;
  border-radius: 999px;
  background: var(--modio-accent);
  color: #041316;
  font-size: 0.68rem;
  font-weight: 700;
  display: inline-flex;
  align-items: center;
  justify-content: center;
}

.tab-content {
  padding-top: 1rem;
}

.tab-empty,
.tab-loading {
  color: var(--modio-text-muted);
}

.tab-loading {
  display: flex;
  align-items: center;
  gap: 0.6rem;
}

.tab-error {
  margin: 0;
}

.mod-description :deep(p) {
  margin: 0 0 0.75rem;
  line-height: 1.6;
  color: var(--modio-text);
}

.mod-description :deep(p:last-child) {
  margin-bottom: 0;
}

.mod-description :deep(a) {
  color: var(--modio-accent);
}

.mod-description :deep(ul),
.mod-description :deep(ol) {
  margin: 0 0 0.75rem;
  padding-left: 1.25rem;
}

.dependencies-toolbar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 1rem;
  margin-bottom: 0.85rem;
}

.dependencies-hint {
  margin: 0;
  color: var(--modio-text-muted);
  font-size: 0.85rem;
}

.sort-control {
  display: inline-flex;
  align-items: center;
  gap: 0.35rem;
  flex-shrink: 0;
}

.sort-icon {
  color: var(--modio-text-muted);
  font-size: 0.85rem;
}

.sort-select {
  padding: 0.35rem 0.55rem;
  font-size: 0.82rem;
  background: var(--modio-surface-raised);
}

.dependency-list {
  list-style: none;
  margin: 0;
  padding: 0;
  display: flex;
  flex-direction: column;
  gap: 0.65rem;
}

.dependency-card {
  display: grid;
  grid-template-columns: minmax(0, 1fr) auto;
  gap: 0.85rem;
  align-items: center;
  padding: 0.75rem;
  border-radius: var(--modio-radius);
  border: 1px solid var(--modio-border);
  background: var(--modio-surface);
  transition:
    border-color 0.2s ease,
    background-color 0.2s ease;
}

.dependency-card:hover {
  border-color: rgba(7, 193, 216, 0.45);
  background: var(--modio-surface-raised);
}

.dependency-card-link {
  display: grid;
  grid-template-columns: 4.5rem minmax(0, 1fr);
  gap: 0.85rem;
  align-items: center;
  color: inherit;
  text-decoration: none;
}

.dependency-card-link:hover {
  color: inherit;
}

.dependency-thumb {
  aspect-ratio: 16 / 9;
  border-radius: var(--modio-radius-sm);
  overflow: hidden;
  background: var(--modio-surface-raised);
}

.dependency-thumb img {
  width: 100%;
  height: 100%;
  object-fit: cover;
  display: block;
}

.dependency-thumb-fallback {
  width: 100%;
  height: 100%;
  background: linear-gradient(
    135deg,
    var(--modio-surface-raised),
    var(--modio-surface-hover)
  );
}

.dependency-info h3 {
  margin: 0 0 0.2rem;
  font-size: 0.95rem;
  font-weight: 600;
}

.dependency-meta {
  margin: 0;
  font-size: 0.8rem;
  color: var(--modio-text-muted);
}

.dependency-actions {
  display: flex;
  flex-direction: column;
  align-items: flex-end;
  gap: 0.45rem;
}

.dependency-size {
  font-size: 0.82rem;
  color: var(--modio-text);
  white-space: nowrap;
}

.mod-detail-sidebar {
  position: sticky;
  top: 1rem;
  display: flex;
  flex-direction: column;
  gap: 1rem;
}

.sidebar-title {
  margin: 0;
  font-size: 1.5rem;
  font-weight: 700;
  letter-spacing: -0.02em;
  line-height: 1.2;
}

.sidebar-install-hint {
  margin: 0;
  font-size: 0.82rem;
  color: var(--modio-danger);
}

.subscribe-button {
  display: block;
  width: 100%;
  padding: 0.7rem 1rem;
  border-radius: var(--modio-radius-sm);
  border: 1px solid var(--modio-accent);
  color: var(--modio-accent);
  text-align: center;
  font-weight: 600;
  text-decoration: none;
  transition:
    background-color 0.2s ease,
    color 0.2s ease;
}

.subscribe-button:hover {
  background: rgba(7, 193, 216, 0.12);
  color: var(--modio-accent);
}

.sidebar-section {
  padding-top: 0.25rem;
}

.sidebar-heading {
  margin: 0 0 0.55rem;
  font-size: 0.72rem;
  font-weight: 700;
  letter-spacing: 0.08em;
  text-transform: uppercase;
  color: var(--modio-text-muted);
}

.ratings-header {
  display: flex;
  align-items: baseline;
  justify-content: space-between;
  gap: 0.5rem;
  margin-bottom: 0.45rem;
}

.ratings-label {
  font-size: 0.9rem;
  font-weight: 600;
}

.ratings-percent {
  color: var(--modio-accent);
  font-weight: 700;
}

.ratings-bar {
  height: 0.35rem;
  border-radius: 999px;
  background: var(--modio-surface-raised);
  overflow: hidden;
  margin-bottom: 0.65rem;
}

.ratings-bar-fill {
  height: 100%;
  border-radius: inherit;
  background: var(--modio-accent);
}

.ratings-votes {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 0.5rem;
}

.vote {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: 0.35rem;
  padding: 0.45rem 0.5rem;
  border-radius: var(--modio-radius-sm);
  font-size: 0.82rem;
  font-weight: 600;
}

.vote-like {
  border: 1px solid rgba(74, 222, 128, 0.45);
  color: var(--modio-success);
}

.vote-dislike {
  border: 1px solid rgba(248, 113, 113, 0.45);
  color: var(--modio-danger);
}

.vote-icon {
  font-size: 0.9rem;
}

.stats-list {
  margin: 0;
  display: flex;
  flex-direction: column;
  gap: 0.55rem;
}

.stat-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 0.75rem;
  font-size: 0.85rem;
}

.stat-row dt {
  margin: 0;
  color: var(--modio-text-muted);
  font-weight: 500;
}

.stat-row dd {
  margin: 0;
  color: var(--modio-text);
  font-weight: 600;
  text-align: right;
}

.stat-id {
  display: inline-flex;
  align-items: center;
  gap: 0.35rem;
}

.copy-id {
  width: 1.35rem;
  height: 1.35rem;
  padding: 0;
  border-radius: var(--modio-radius-sm);
  border: 1px solid var(--modio-border);
  background: var(--modio-surface-raised);
  color: var(--modio-text-muted);
  font-size: 0.72rem;
  line-height: 1;
}

.copy-id:hover:not(:disabled) {
  color: var(--modio-accent);
  border-color: var(--modio-accent);
  background: var(--modio-surface-hover);
}

.tag-list {
  display: flex;
  flex-wrap: wrap;
  gap: 0.4rem;
}

.tag-pill {
  padding: 0.2rem 0.55rem;
  border-radius: 999px;
  border: 1px solid rgba(7, 193, 216, 0.35);
  color: var(--modio-accent);
  font-size: 0.75rem;
  font-weight: 600;
}

.creator-row {
  display: grid;
  grid-template-columns: auto 1fr auto;
  align-items: center;
  gap: 0.65rem;
  padding: 0.55rem 0;
  color: inherit;
  text-decoration: none;
}

.creator-row:hover .creator-name {
  color: var(--modio-accent);
}

.creator-avatar {
  width: 2rem;
  height: 2rem;
  border-radius: 999px;
  object-fit: cover;
}

.creator-avatar-fallback {
  background: var(--modio-surface-raised);
  border: 1px solid var(--modio-border);
}

.creator-name {
  font-size: 0.9rem;
  font-weight: 600;
}

.creator-link-icon {
  color: var(--modio-text-muted);
  font-size: 0.85rem;
}

.homepage-link {
  font-size: 0.85rem;
  font-weight: 600;
}

.state {
  display: flex;
  flex-direction: column;
  align-items: flex-start;
  gap: 0.75rem;
  padding: 3rem 0;
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

@media (max-width: 900px) {
  .mod-detail-layout {
    grid-template-columns: 1fr;
  }

  .mod-detail-sidebar {
    position: static;
  }
}

@media (max-width: 640px) {
  .dependencies-toolbar {
    flex-direction: column;
    align-items: flex-start;
  }

  .dependency-card {
    grid-template-columns: 1fr;
  }

  .dependency-card-link {
    grid-template-columns: 3.5rem minmax(0, 1fr);
  }

  .dependency-actions {
    flex-direction: row;
    align-items: center;
    justify-content: space-between;
    width: 100%;
  }
}
</style>
