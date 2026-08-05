<script setup lang="ts">
import { useVirtualizer } from "@tanstack/vue-virtual";
import type { ModSummary } from "~/composables/useMods";

const props = defineProps<{
  mods: ModSummary[];
  showInstall?: boolean;
}>();

const emit = defineEmits<{
  install: [modId: number];
  uninstall: [modId: number, modName: string];
}>();

const MIN_COLUMN_REM = 15.5;
const GAP_REM = 1;
const ESTIMATED_ROW_HEIGHT = 320;

const containerRef = ref<HTMLElement | null>(null);
const scrollElement = shallowRef<HTMLElement | null>(null);
const columnCount = ref(1);

function remToPx(rem: number) {
  if (typeof window === "undefined") return rem * 16;
  const rootSize = Number.parseFloat(
    getComputedStyle(document.documentElement).fontSize || "16",
  );
  return rem * (Number.isFinite(rootSize) ? rootSize : 16);
}

function updateColumns() {
  const el = containerRef.value;
  if (!el) return;
  const width = el.clientWidth;
  const minCol = remToPx(MIN_COLUMN_REM);
  const gap = remToPx(GAP_REM);
  columnCount.value = Math.max(1, Math.floor((width + gap) / (minCol + gap)));
}

const rowCount = computed(() =>
  props.mods.length === 0
    ? 0
    : Math.ceil(props.mods.length / columnCount.value),
);

function modsForRow(rowIndex: number) {
  const cols = columnCount.value;
  const start = rowIndex * cols;
  return props.mods.slice(start, start + cols);
}

const virtualizerOptions = computed(() => ({
  count: rowCount.value,
  getScrollElement: () => scrollElement.value,
  estimateSize: () => ESTIMATED_ROW_HEIGHT,
  overscan: 2,
  gap: remToPx(GAP_REM),
  measureElement:
    typeof window !== "undefined" &&
    !navigator.userAgent.includes("Firefox")
      ? (element: Element) => element.getBoundingClientRect().height
      : undefined,
}));

const rowVirtualizer = useVirtualizer(virtualizerOptions);

const virtualRows = computed(() => rowVirtualizer.value.getVirtualItems());
const totalSize = computed(() => rowVirtualizer.value.getTotalSize());

function measureRow(
  el: Element | ComponentPublicInstance | null,
) {
  if (el === null) {
    rowVirtualizer.value.measureElement(null);
    return;
  }
  if (!(el instanceof Element)) return;
  rowVirtualizer.value.measureElement(el);
}

let resizeObserver: ResizeObserver | undefined;

onMounted(() => {
  scrollElement.value =
    containerRef.value?.closest(".app-main") ??
    (document.querySelector(".app-main") as HTMLElement | null);
  updateColumns();
  if (containerRef.value) {
    resizeObserver = new ResizeObserver(() => {
      updateColumns();
    });
    resizeObserver.observe(containerRef.value);
  }
});

onBeforeUnmount(() => {
  resizeObserver?.disconnect();
});

watch(columnCount, () => {
  rowVirtualizer.value.measure();
});
</script>

<template>
  <div ref="containerRef" class="mod-virtual-grid" role="list">
    <div
      class="mod-virtual-grid-inner"
      :style="{ height: `${totalSize}px` }"
    >
      <div
        v-for="virtualRow in virtualRows"
        :key="virtualRow.index"
        :data-index="virtualRow.index"
        :ref="measureRow"
        class="mod-virtual-row"
        role="presentation"
        :style="{
          transform: `translateY(${virtualRow.start}px)`,
          gridTemplateColumns: `repeat(${columnCount}, minmax(0, 1fr))`,
        }"
      >
        <div
          v-for="mod in modsForRow(virtualRow.index)"
          :key="mod.id"
          class="mod-virtual-cell"
          role="listitem"
        >
          <ModCard
            :mod="mod"
            :show-install="showInstall"
            @install="emit('install', mod.id)"
            @uninstall="emit('uninstall', mod.id, mod.name)"
          />
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.mod-virtual-grid {
  width: 100%;
}

.mod-virtual-grid-inner {
  position: relative;
  width: 100%;
}

.mod-virtual-row {
  position: absolute;
  top: 0;
  left: 0;
  width: 100%;
  display: grid;
  gap: 1rem;
  box-sizing: border-box;
}

.mod-virtual-cell {
  min-width: 0;
  height: 100%;
}

.mod-virtual-cell :deep(.mod-card) {
  height: 100%;
}
</style>
