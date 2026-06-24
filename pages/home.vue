<script setup lang="ts">
import { onMounted, ref } from "vue";
import { invoke } from "~/utils/tauri";
import type { ModSort, ModTypeFilter } from "~/composables/useMods";

definePageMeta({ layout: "app" });

interface ModioStatus {
  configured: boolean;
  message?: string;
}

const {
  mods,
  total,
  loading,
  error,
  search,
  modType,
  categoryTags,
  sort,
  sortDir,
  hasMore,
  fetchMods,
  loadMore,
} = useMods();

const { tagOptions, fetchTagOptions } = useModTagOptions();

const modioConfigured = ref(false);
const modioMessage = ref("");

const modTypeOptions: { value: ModTypeFilter; label: string }[] = [
  { value: "all", label: "All types" },
  { value: "plugin", label: "Plugin" },
  { value: "blueprint", label: "Blueprint" },
];

const activeCategoryLabel = computed(() => {
  if (modType.value === "plugin") return "Plugin type";
  if (modType.value === "blueprint") return "Blueprint type";
  return "";
});

const activeCategoryOptions = computed(() => {
  if (!tagOptions.value) return [];
  if (modType.value === "plugin") return tagOptions.value.pluginTypes;
  if (modType.value === "blueprint") return tagOptions.value.blueprintTypes;
  return [];
});

function isCategoryTagSelected(tag: string) {
  return categoryTags.value.includes(tag);
}

function toggleCategoryTag(tag: string) {
  if (isCategoryTagSelected(tag)) {
    categoryTags.value = categoryTags.value.filter((value) => value !== tag);
    return;
  }

  categoryTags.value = [...categoryTags.value, tag];
}

function clearCategoryTags() {
  categoryTags.value = [];
}

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

onMounted(async () => {
  await checkModioStatus();
  if (modioConfigured.value) {
    await Promise.all([fetchTagOptions(), fetchMods()]);
  }
});
</script>

<template>
  <div class="mods-page">
    <header class="page-header">
      <h1>Mods</h1>
    </header>

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
            <span>Type</span>
            <select v-model="modType" aria-label="Filter by mod type">
              <option
                v-for="option in modTypeOptions"
                :key="option.value"
                :value="option.value"
              >
                {{ option.label }}
              </option>
            </select>
          </label>
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

      <section
        v-if="activeCategoryOptions.length"
        class="category-filters"
        :aria-label="`${activeCategoryLabel} filters`"
      >
        <div class="category-filters-header">
          <h2 class="category-filters-title">{{ activeCategoryLabel }}</h2>
          <button
            v-if="categoryTags.length"
            type="button"
            class="link-button clear-tags"
            @click="clearCategoryTags"
          >
            Clear
          </button>
        </div>
        <div class="category-tag-list">
          <button
            v-for="tag in activeCategoryOptions"
            :key="tag"
            type="button"
            class="category-tag"
            :class="{ active: isCategoryTagSelected(tag) }"
            :aria-pressed="isCategoryTagSelected(tag)"
            @click="toggleCategoryTag(tag)"
          >
            {{ tag }}
          </button>
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
        <ModCard :mod="mod" />
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

.category-filters {
  margin-bottom: 0.85rem;
  padding: 1rem 1.1rem;
  border-radius: var(--modio-radius);
  background: var(--modio-surface);
  border: 1px solid var(--modio-border);
}

.category-filters-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 0.75rem;
  margin-bottom: 0.75rem;
}

.category-filters-title {
  margin: 0;
  font-size: 0.75rem;
  font-weight: 600;
  letter-spacing: 0.04em;
  text-transform: uppercase;
  color: var(--modio-text-muted);
}

.clear-tags {
  font-size: 0.82rem;
}

.category-tag-list {
  display: flex;
  flex-wrap: wrap;
  gap: 0.45rem;
}

.category-tag {
  padding: 0.3rem 0.65rem;
  border-radius: 999px;
  border: 1px solid var(--modio-border);
  background: var(--modio-surface-raised);
  color: var(--modio-text-muted);
  font-size: 0.8rem;
  font-weight: 500;
}

.category-tag:hover:not(:disabled) {
  border-color: rgba(7, 193, 216, 0.45);
  color: var(--modio-text);
  background: var(--modio-surface-hover);
}

.category-tag.active {
  border-color: rgba(7, 193, 216, 0.55);
  background: rgba(7, 193, 216, 0.12);
  color: var(--modio-accent);
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

.mods-footer {
  display: flex;
  justify-content: center;
  margin-top: 2rem;
}

@media (max-width: 640px) {
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
