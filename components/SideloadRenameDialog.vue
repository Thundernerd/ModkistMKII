<script setup lang="ts">
const props = defineProps<{
  open: boolean;
  currentName: string;
  busy?: boolean;
}>();

const emit = defineEmits<{
  close: [];
  rename: [name: string];
}>();

const name = ref("");
const inputRef = ref<HTMLInputElement | null>(null);

watch(
  () => props.open,
  (open) => {
    if (open) {
      name.value = props.currentName;
      nextTick(() => {
        inputRef.value?.focus();
        inputRef.value?.select();
      });
    }
  },
);

const canSubmit = computed(
  () => name.value.trim().length > 0 && !props.busy,
);

function submit() {
  if (!canSubmit.value) return;
  emit("rename", name.value.trim());
}
</script>

<template>
  <div
    v-if="open"
    class="sideload-rename-backdrop"
    @click.self="emit('close')"
  >
    <div
      class="sideload-rename-dialog"
      role="dialog"
      aria-modal="true"
      aria-label="Rename sideloaded mod"
    >
      <h2 class="sideload-rename-title">Rename</h2>
      <p class="hint sideload-rename-desc">
        Choose a new name for this sideloaded mod.
      </p>

      <form class="sideload-rename-form" @submit.prevent="submit">
        <label class="sideload-rename-label" for="sideload-rename-input">
          Name
        </label>
        <input
          id="sideload-rename-input"
          ref="inputRef"
          v-model="name"
          type="text"
          class="sideload-rename-input"
          autocomplete="off"
          :disabled="busy"
        />

        <div class="sideload-rename-actions">
          <button
            type="button"
            class="btn-secondary"
            :disabled="busy"
            @click="emit('close')"
          >
            Cancel
          </button>
          <button type="submit" :disabled="!canSubmit">
            {{ busy ? "Renaming…" : "Rename" }}
          </button>
        </div>
      </form>
    </div>
  </div>
</template>

<style scoped>
.sideload-rename-backdrop {
  position: fixed;
  inset: 0;
  z-index: 100;
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 1rem;
  background: rgba(0, 0, 0, 0.55);
}

.sideload-rename-dialog {
  width: min(100%, 28rem);
  padding: 1.25rem;
  border-radius: var(--modio-radius);
  background: var(--modio-surface);
  border: 1px solid var(--modio-border);
  box-shadow: var(--modio-shadow);
}

.sideload-rename-title {
  margin: 0 0 0.5rem;
  font-size: 1.1rem;
  font-weight: 600;
}

.sideload-rename-desc {
  margin: 0 0 1rem;
}

.sideload-rename-label {
  display: block;
  margin-bottom: 0.35rem;
  font-size: 0.85rem;
  font-weight: 600;
}

.sideload-rename-input {
  width: 100%;
  box-sizing: border-box;
  margin-bottom: 1.25rem;
  padding: 0.5rem 0.65rem;
  border-radius: var(--modio-radius-sm);
  border: 1px solid var(--modio-border);
  background: var(--modio-surface-raised);
  color: var(--modio-text);
}

.sideload-rename-actions {
  display: flex;
  flex-wrap: wrap;
  justify-content: flex-end;
  gap: 0.65rem;
}
</style>
