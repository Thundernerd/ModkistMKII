<script setup lang="ts">
import type { SideloadTargetKind } from "~/composables/useSideload";

defineProps<{
  open: boolean;
  folderName: string;
}>();

const emit = defineEmits<{
  close: [];
  select: [targetKind: SideloadTargetKind];
}>();
</script>

<template>
  <div
    v-if="open"
    class="sideload-target-backdrop"
    @click.self="emit('close')"
  >
    <div
      class="sideload-target-dialog panel"
      role="dialog"
      aria-modal="true"
      aria-label="Choose sideload destination"
    >
      <h2 class="sideload-target-title">Choose destination</h2>
      <p class="hint sideload-target-desc">
        The archive <strong>{{ folderName }}</strong> contains both blueprint
        and plugin files. Where should it be installed?
      </p>

      <div class="sideload-target-actions">
        <button type="button" class="btn-secondary" @click="emit('close')">
          Cancel
        </button>
        <button
          type="button"
          @click="emit('select', 'plugins')"
        >
          Plugins
        </button>
        <button
          type="button"
          @click="emit('select', 'blueprints')"
        >
          Blueprints
        </button>
      </div>
    </div>
  </div>
</template>

<style scoped>
.sideload-target-backdrop {
  position: fixed;
  inset: 0;
  z-index: 100;
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 1rem;
  background: rgba(0, 0, 0, 0.55);
}

.sideload-target-dialog {
  width: min(100%, 28rem);
  padding: 1.25rem;
}

.sideload-target-title {
  margin: 0 0 0.5rem;
  font-size: 1.1rem;
  font-weight: 600;
}

.sideload-target-desc {
  margin: 0 0 1.25rem;
}

.sideload-target-actions {
  display: flex;
  flex-wrap: wrap;
  justify-content: flex-end;
  gap: 0.65rem;
}
</style>
