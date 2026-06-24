<script setup lang="ts">
import type { ModSort } from "~/composables/useMods";
import { SORT_OPTIONS } from "~/composables/useModFilters";

const sort = defineModel<ModSort>("sort", { required: true });
const sortDir = defineModel<"asc" | "desc">("sortDir", { required: true });

const rootRef = ref<HTMLElement | null>(null);
const menuOpen = ref(false);

const currentLabel = computed(
  () => SORT_OPTIONS.find((option) => option.value === sort.value)?.label ?? "Sort",
);

const directionLabel = computed(() =>
  sortDir.value === "desc" ? "Sort descending" : "Sort ascending",
);

function toggleMenu() {
  menuOpen.value = !menuOpen.value;
}

function selectSort(value: ModSort) {
  sort.value = value;
  menuOpen.value = false;
}

function toggleDirection() {
  sortDir.value = sortDir.value === "desc" ? "asc" : "desc";
}

function onDocumentClick(event: MouseEvent) {
  if (!menuOpen.value) return;
  if (rootRef.value?.contains(event.target as Node)) return;
  menuOpen.value = false;
}

function onDocumentKeydown(event: KeyboardEvent) {
  if (event.key === "Escape") menuOpen.value = false;
}

onMounted(() => {
  document.addEventListener("click", onDocumentClick);
  document.addEventListener("keydown", onDocumentKeydown);
});

onUnmounted(() => {
  document.removeEventListener("click", onDocumentClick);
  document.removeEventListener("keydown", onDocumentKeydown);
});
</script>

<template>
  <div ref="rootRef" class="sort-control">
    <div class="sort-control-group">
      <button
        type="button"
        class="sort-trigger"
        :aria-expanded="menuOpen"
        aria-haspopup="listbox"
        :aria-label="`Sort by ${currentLabel}`"
        @click="toggleMenu"
      >
        <span class="sort-trigger-label">{{ currentLabel }}</span>
        <svg
          class="sort-chevron"
          :class="{ open: menuOpen }"
          width="12"
          height="12"
          viewBox="0 0 12 12"
          aria-hidden="true"
        >
          <path
            d="M3 4.5 6 7.5 9 4.5"
            fill="none"
            stroke="currentColor"
            stroke-width="1.5"
            stroke-linecap="round"
            stroke-linejoin="round"
          />
        </svg>
      </button>

      <button
        type="button"
        class="sort-dir-btn"
        :aria-label="directionLabel"
        :title="directionLabel"
        @click="toggleDirection"
      >
        <svg
          v-if="sortDir === 'desc'"
          width="14"
          height="14"
          viewBox="0 0 14 14"
          aria-hidden="true"
        >
          <path
            d="M7 2.5v9M7 11.5 4.5 9M7 11.5 9.5 9"
            fill="none"
            stroke="currentColor"
            stroke-width="1.5"
            stroke-linecap="round"
            stroke-linejoin="round"
          />
        </svg>
        <svg
          v-else
          width="14"
          height="14"
          viewBox="0 0 14 14"
          aria-hidden="true"
        >
          <path
            d="M7 11.5v-9M7 2.5 9.5 5M7 2.5 4.5 5"
            fill="none"
            stroke="currentColor"
            stroke-width="1.5"
            stroke-linecap="round"
            stroke-linejoin="round"
          />
        </svg>
      </button>
    </div>

    <ul
      v-if="menuOpen"
      class="sort-menu"
      role="listbox"
      aria-label="Sort by"
    >
      <li
        v-for="option in SORT_OPTIONS"
        :key="option.value"
        role="option"
        :aria-selected="sort === option.value"
      >
        <button
          type="button"
          class="sort-menu-item"
          :class="{ active: sort === option.value }"
          @click="selectSort(option.value)"
        >
          {{ option.label }}
        </button>
      </li>
    </ul>
  </div>
</template>

<style scoped>
.sort-control {
  position: relative;
  flex-shrink: 0;
}

.sort-control-group {
  display: inline-flex;
  align-items: stretch;
  height: 100%;
  border-radius: var(--modio-radius-sm);
  border: 1px solid var(--modio-border);
  background: var(--modio-surface-raised);
  overflow: hidden;
  box-sizing: border-box;
}

.sort-trigger,
.sort-dir-btn {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: 0.35rem;
  height: 100%;
  padding: 0 0.75rem;
  border: none;
  background: transparent;
  color: var(--modio-text);
  font-size: 0.85rem;
  font-weight: 600;
  box-shadow: none;
}

.sort-trigger:hover:not(:disabled),
.sort-dir-btn:hover:not(:disabled) {
  background: var(--modio-surface-hover);
  color: var(--modio-text);
  border-color: transparent;
}

.sort-trigger-label {
  white-space: nowrap;
}

.sort-chevron {
  flex-shrink: 0;
  color: var(--modio-text-muted);
  transition: transform 0.15s ease;
}

.sort-chevron.open {
  transform: rotate(180deg);
}

.sort-dir-btn {
  padding-inline: 0.6rem;
  border-left: 1px solid var(--modio-border);
  color: var(--modio-text-muted);
}

.sort-dir-btn:hover:not(:disabled) {
  color: var(--modio-accent);
}

.sort-menu {
  position: absolute;
  top: calc(100% + 0.35rem);
  left: 0;
  z-index: 20;
  min-width: 11rem;
  margin: 0;
  padding: 0.3rem;
  list-style: none;
  border-radius: var(--modio-radius-sm);
  border: 1px solid var(--modio-border);
  background: var(--modio-surface);
  box-shadow: var(--modio-shadow);
}

.sort-menu-item {
  display: block;
  width: 100%;
  padding: 0.45rem 0.65rem;
  border: none;
  border-radius: calc(var(--modio-radius-sm) - 2px);
  background: transparent;
  color: var(--modio-text-muted);
  font-size: 0.85rem;
  font-weight: 500;
  text-align: left;
  box-shadow: none;
}

.sort-menu-item:hover:not(:disabled) {
  background: var(--modio-surface-hover);
  color: var(--modio-text);
  border-color: transparent;
}

.sort-menu-item.active {
  background: rgba(7, 193, 216, 0.12);
  color: var(--modio-accent);
}

@media (max-width: 640px) {
  .sort-control {
    width: 100%;
  }

  .sort-control-group {
    flex: 1;
  }

  .sort-trigger {
    flex: 1;
    justify-content: space-between;
  }

  .sort-menu {
    left: 0;
    right: 0;
    min-width: 0;
  }
}
</style>
