<script setup lang="ts">
import { onMounted, ref } from "vue";
import { invoke } from "~/utils/tauri";
import type { ModSort } from "~/composables/useMods";
import { ensureGamePath } from "~/utils/authNavigation";

interface ModioStatus {
  configured: boolean;
  message?: string;
}

const { authStatus, refreshAuthStatus, logout } = useModioAuth();
const {
  mods,
  total,
  loading,
  error,
  search,
  sort,
  sortDir,
  hasMore,
  fetchMods,
  loadMore,
} = useMods();

const modioConfigured = ref(false);
const modioMessage = ref("");

const sortOptions: { value: ModSort; label: string }[] = [
  { value: "recentlyAdded", label: "Recently added" },
  { value: "lastUpdated", label: "Last updated" },
  { value: "trending", label: "Trending" },
  { value: "mostPopular", label: "Most popular" },
  { value: "mostSubscribers", label: "Most subscribers" },
  { value: "highestRated", label: "Highest rated" },
  { value: "alphabetical", label: "Alphabetical" },
];

async function checkModioStatus() {
  const status = await invoke<ModioStatus>("modio_status");
  modioConfigured.value = status.configured;
  modioMessage.value = status.message ?? "";
}

async function handleLogout() {
  await logout();
  await navigateTo("/");
}

function formatDate(iso: string) {
  if (!iso) return "";
  return new Date(iso).toLocaleDateString();
}

function formatCount(value: number) {
  return value.toLocaleString();
}

onMounted(async () => {
  await refreshAuthStatus();
  if (!(await ensureGamePath())) {
    return;
  }
  await checkModioStatus();
  if (modioConfigured.value) {
    await fetchMods();
  }
});
</script>

<template>
  <div class="mods-shell">
    <header class="mods-topbar">
      <div class="mods-brand">
        <span class="mods-brand-mark" aria-hidden="true" />
        <h1>Mods</h1>
      </div>
      <nav class="auth-bar">
        <template v-if="authStatus.loggedIn">
          <span class="auth-user">{{ authStatus.username }}</span>
          <button type="button" class="link-button" @click="handleLogout">
            Log out
          </button>
        </template>
        <NuxtLink v-else to="/" class="link-button">Sign in</NuxtLink>
      </nav>
    </header>

    <main class="mods-page">
      <p v-if="!modioConfigured" class="hint mods-hint">
        {{ modioMessage || "Configure mod.io in .env (see .env.example)." }}
      </p>

      <template v-else>
        <section class="mods-toolbar" aria-label="Filter and sort mods">
          <label class="search-field">
            <span class="search-icon" aria-hidden="true">⌕</span>
            <input
              v-model="search"
              type="search"
              placeholder="Search mods"
              aria-label="Search mods"
            />
          </label>
          <div class="toolbar-controls">
            <label class="control-label">
              <span>Sort by</span>
              <select v-model="sort" aria-label="Sort by">
                <option
                  v-for="option in sortOptions"
                  :key="option.value"
                  :value="option.value"
                >
                  {{ option.label }}
                </option>
              </select>
            </label>
            <label class="control-label">
              <span>Order</span>
              <select v-model="sortDir" aria-label="Sort direction">
                <option value="desc">Descending</option>
                <option value="asc">Ascending</option>
              </select>
            </label>
          </div>
        </section>

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
          <a
            :href="mod.profileUrl"
            target="_blank"
            rel="noopener noreferrer"
            class="mod-card"
          >
            <div class="mod-thumb">
              <img
                v-if="mod.logoUrl"
                :src="mod.logoUrl"
                :alt="`${mod.name} thumbnail`"
                loading="lazy"
              />
              <div v-else class="mod-thumb-fallback" />
            </div>
            <div class="mod-content">
              <h2 class="mod-name">{{ mod.name }}</h2>
              <p class="mod-summary">{{ mod.summary }}</p>
              <div class="mod-stats">
                <span class="stat">
                  <span class="stat-icon" aria-hidden="true">↓</span>
                  {{ formatCount(mod.downloadsTotal) }}
                </span>
                <span class="stat">
                  <span class="stat-icon" aria-hidden="true">★</span>
                  {{ formatCount(mod.subscribersTotal) }}
                </span>
                <span v-if="mod.popularityRank" class="stat">
                  #{{ mod.popularityRank }}
                </span>
              </div>
              <div v-if="mod.tags.length" class="mod-tags">
                <span v-for="tag in mod.tags" :key="tag" class="tag">{{
                  tag
                }}</span>
              </div>
              <p v-if="mod.dateUpdated" class="mod-updated">
                Updated {{ formatDate(mod.dateUpdated) }}
              </p>
            </div>
          </a>
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
    </main>
  </div>
</template>

<style scoped>
.mods-shell {
  min-height: 100vh;
  background:
    radial-gradient(
      ellipse 80% 50% at 50% -20%,
      rgba(7, 193, 216, 0.12),
      transparent
    ),
    var(--modio-bg);
}

.mods-topbar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 1rem;
  padding: 1rem 1.5rem;
  border-bottom: 1px solid var(--modio-border);
  background: rgba(18, 18, 20, 0.92);
  backdrop-filter: blur(8px);
  position: sticky;
  top: 0;
  z-index: 10;
}

.mods-brand {
  display: flex;
  align-items: center;
  gap: 0.75rem;
}

.mods-brand-mark {
  width: 0.35rem;
  height: 1.75rem;
  border-radius: 999px;
  background: var(--modio-accent);
}

.mods-brand h1 {
  margin: 0;
  font-size: 1.35rem;
  font-weight: 700;
  letter-spacing: -0.02em;
}

.auth-bar {
  display: flex;
  align-items: center;
  gap: 1rem;
  font-size: 0.9rem;
}

.auth-bar a.link-button {
  text-decoration: none;
}

.auth-user {
  color: var(--modio-text-muted);
}

.mods-page {
  margin: 0 auto;
  max-width: 72rem;
  padding: 1.5rem 1.5rem 3rem;
}

.mods-hint {
  padding: 2rem;
  text-align: center;
  border: 1px dashed var(--modio-border);
  border-radius: var(--modio-radius);
  background: var(--modio-surface);
}

.mods-toolbar {
  display: flex;
  flex-wrap: wrap;
  gap: 0.75rem;
  margin-bottom: 0.75rem;
}

.search-field {
  flex: 1 1 16rem;
  position: relative;
  display: flex;
  align-items: center;
}

.search-field input {
  width: 100%;
  padding-left: 2.25rem;
}

.search-icon {
  position: absolute;
  left: 0.85rem;
  color: var(--modio-text-muted);
  font-size: 1rem;
  pointer-events: none;
}

.toolbar-controls {
  display: flex;
  flex-wrap: wrap;
  gap: 0.75rem;
}

.control-label {
  display: flex;
  flex-direction: column;
  gap: 0.3rem;
  font-size: 0.75rem;
  color: var(--modio-text-muted);
  text-transform: uppercase;
  letter-spacing: 0.04em;
}

.control-label select {
  min-width: 9rem;
  text-transform: none;
  letter-spacing: normal;
  font-size: 0.9rem;
}

.mods-count {
  margin: 0 0 1.25rem;
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

.mod-card {
  display: flex;
  flex-direction: column;
  height: 100%;
  color: inherit;
  background: var(--modio-surface);
  border: 1px solid var(--modio-border);
  border-radius: var(--modio-radius);
  overflow: hidden;
  transition:
    transform 0.2s ease,
    border-color 0.2s ease,
    box-shadow 0.2s ease;
}

.mod-card:hover {
  transform: translateY(-2px);
  border-color: rgba(7, 193, 216, 0.45);
  box-shadow: var(--modio-shadow);
  color: inherit;
}

.mod-thumb {
  aspect-ratio: 16 / 9;
  background: var(--modio-surface-raised);
  overflow: hidden;
}

.mod-thumb img {
  width: 100%;
  height: 100%;
  object-fit: cover;
  display: block;
}

.mod-thumb-fallback {
  width: 100%;
  height: 100%;
  background: linear-gradient(
    135deg,
    var(--modio-surface-raised),
    var(--modio-surface-hover)
  );
}

.mod-content {
  display: flex;
  flex-direction: column;
  gap: 0.45rem;
  padding: 0.85rem 0.9rem 1rem;
  flex: 1;
}

.mod-name {
  margin: 0;
  font-size: 0.98rem;
  font-weight: 600;
  line-height: 1.3;
  display: -webkit-box;
  -webkit-line-clamp: 2;
  -webkit-box-orient: vertical;
  overflow: hidden;
}

.mod-summary {
  margin: 0;
  font-size: 0.82rem;
  line-height: 1.45;
  color: var(--modio-text-subtle);
  display: -webkit-box;
  -webkit-line-clamp: 2;
  -webkit-box-orient: vertical;
  overflow: hidden;
  flex: 1;
}

.mod-stats {
  display: flex;
  flex-wrap: wrap;
  gap: 0.55rem;
  margin-top: 0.15rem;
}

.stat {
  display: inline-flex;
  align-items: center;
  gap: 0.25rem;
  padding: 0.15rem 0.45rem;
  border-radius: 999px;
  background: var(--modio-surface-raised);
  color: var(--modio-text-muted);
  font-size: 0.75rem;
  font-weight: 500;
}

.stat-icon {
  color: var(--modio-accent);
  font-size: 0.7rem;
}

.mod-tags {
  display: flex;
  flex-wrap: wrap;
  gap: 0.35rem;
}

.tag {
  padding: 0.12rem 0.5rem;
  border-radius: 999px;
  border: 1px solid var(--modio-border);
  background: transparent;
  color: var(--modio-text-muted);
  font-size: 0.72rem;
}

.mod-updated {
  margin: 0;
  font-size: 0.72rem;
  color: var(--modio-text-subtle);
}

.mods-footer {
  display: flex;
  justify-content: center;
  margin-top: 2rem;
}

@media (max-width: 640px) {
  .mods-topbar {
    flex-direction: column;
    align-items: flex-start;
  }

  .toolbar-controls {
    width: 100%;
  }

  .control-label {
    flex: 1;
  }

  .control-label select {
    width: 100%;
  }
}
</style>
