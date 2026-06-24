<script setup lang="ts">
import type { ModSort, ModTypeFilter } from "~/composables/useMods";
import { MOD_TYPE_OPTIONS, SORT_OPTIONS } from "~/composables/useModFilters";

const search = defineModel<string>("search", { required: true });
const modType = defineModel<ModTypeFilter>("modType", { required: true });
const sort = defineModel<ModSort>("sort", { required: true });
const sortDir = defineModel<"asc" | "desc">("sortDir", { required: true });

defineProps<{
  categoryOptions: string[];
  categoryLabel: string;
  hasActiveFilters: boolean;
  selectedCategoryTags: string[];
}>();

const emit = defineEmits<{
  toggleCategoryTag: [tag: string];
  clearCategoryTags: [];
  clearFilters: [];
}>();
</script>

<template>
  <section class="mod-filters" aria-label="Filter and sort mods">
    <div class="mod-filters-primary">
      <label class="search-field">
        <span class="search-icon" aria-hidden="true">⌕</span>
        <input
          v-model="search"
          type="search"
          placeholder="Search mods"
          aria-label="Search mods"
        />
      </label>

      <div
        class="type-toggle"
        role="group"
        aria-label="Filter by mod type"
      >
        <button
          v-for="option in MOD_TYPE_OPTIONS"
          :key="option.value"
          type="button"
          class="type-toggle-btn"
          :class="{ active: modType === option.value }"
          :aria-pressed="modType === option.value"
          @click="modType = option.value"
        >
          {{ option.label }}
        </button>
      </div>
    </div>

    <div class="mod-filters-secondary">
      <label class="control-label">
        <span>Sort by</span>
        <select v-model="sort" aria-label="Sort by">
          <option
            v-for="option in SORT_OPTIONS"
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
      <button
        v-if="hasActiveFilters"
        type="button"
        class="link-button clear-filters"
        @click="emit('clearFilters')"
      >
        Reset filters
      </button>
    </div>

    <div v-if="categoryOptions.length" class="mod-filters-categories">
      <div class="mod-filters-categories-header">
        <span class="mod-filters-label">{{ categoryLabel }}</span>
        <button
          v-if="selectedCategoryTags.length"
          type="button"
          class="link-button"
          @click="emit('clearCategoryTags')"
        >
          Clear
        </button>
      </div>
      <div class="category-tag-list">
        <button
          v-for="tag in categoryOptions"
          :key="tag"
          type="button"
          class="category-tag"
          :class="{ active: selectedCategoryTags.includes(tag) }"
          :aria-pressed="selectedCategoryTags.includes(tag)"
          @click="emit('toggleCategoryTag', tag)"
        >
          {{ tag }}
        </button>
      </div>
    </div>
  </section>
</template>

<style scoped>
.mod-filters {
  display: flex;
  flex-direction: column;
  gap: 0.85rem;
  margin-bottom: 0.85rem;
  padding: 1rem 1.1rem;
  border-radius: var(--modio-radius);
  background: var(--modio-surface);
  border: 1px solid var(--modio-border);
}

.mod-filters-primary {
  display: flex;
  flex-wrap: wrap;
  gap: 0.75rem;
  align-items: center;
}

.search-field {
  flex: 1 1 14rem;
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

.type-toggle {
  display: inline-flex;
  padding: 0.2rem;
  border-radius: var(--modio-radius-sm);
  background: var(--modio-surface-raised);
  border: 1px solid var(--modio-border);
}

.type-toggle-btn {
  padding: 0.45rem 0.85rem;
  border: none;
  border-radius: calc(var(--modio-radius-sm) - 2px);
  background: transparent;
  color: var(--modio-text-muted);
  font-size: 0.85rem;
  font-weight: 600;
}

.type-toggle-btn:hover:not(:disabled) {
  color: var(--modio-text);
  background: var(--modio-surface-hover);
}

.type-toggle-btn.active {
  background: rgba(7, 193, 216, 0.14);
  color: var(--modio-accent);
}

.mod-filters-categories {
  display: flex;
  flex-direction: column;
  gap: 0.55rem;
  padding-top: 0.15rem;
  border-top: 1px solid var(--modio-border);
}

.mod-filters-categories-header,
.mod-filters-secondary {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: 0.75rem;
}

.mod-filters-categories-header {
  justify-content: space-between;
}

.mod-filters-label,
.control-label span {
  font-size: 0.75rem;
  font-weight: 600;
  color: var(--modio-text-muted);
  text-transform: uppercase;
  letter-spacing: 0.04em;
}

.control-label {
  display: flex;
  flex-direction: column;
  gap: 0.3rem;
}

.control-label select {
  min-width: 9rem;
  font-size: 0.9rem;
}

.clear-filters {
  margin-left: auto;
  font-size: 0.82rem;
}

.category-tag-list {
  display: flex;
  flex-wrap: wrap;
  gap: 0.4rem;
}

.category-tag {
  padding: 0.28rem 0.6rem;
  border-radius: 999px;
  border: 1px solid var(--modio-border);
  background: var(--modio-surface-raised);
  color: var(--modio-text-muted);
  font-size: 0.78rem;
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

@media (max-width: 640px) {
  .mod-filters-primary,
  .mod-filters-secondary {
    flex-direction: column;
    align-items: stretch;
  }

  .type-toggle {
    width: 100%;
  }

  .type-toggle-btn {
    flex: 1;
  }

  .control-label select {
    width: 100%;
  }

  .clear-filters {
    margin-left: 0;
  }
}
</style>
