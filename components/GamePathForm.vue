<script setup lang="ts">
import { onMounted, ref } from "vue";
import { open } from "@tauri-apps/plugin-dialog";
import { useGamePath } from "~/composables/useGamePath";

const props = withDefaults(
  defineProps<{
    submitLabel?: string;
    inputId?: string;
  }>(),
  {
    submitLabel: "Save",
    inputId: "game-path",
  },
);

const emit = defineEmits<{
  saved: [];
}>();

const { gamePathStatus, refreshGamePathStatus, setGamePath } = useGamePath();

const path = ref("");
const loading = ref(false);
const error = ref("");

async function browseFolder() {
  error.value = "";
  const selected = await open({
    directory: true,
    multiple: false,
    title: "Select Zeepkist game directory",
  });

  if (typeof selected === "string") {
    path.value = selected;
  }
}

async function submitPath() {
  if (!path.value.trim()) {
    error.value = "Enter your Zeepkist game directory.";
    return;
  }

  loading.value = true;
  error.value = "";

  try {
    await setGamePath(path.value.trim());
    if (gamePathStatus.value.valid) {
      emit("saved");
    }
  } catch (err) {
    error.value = String(err);
  } finally {
    loading.value = false;
  }
}

onMounted(async () => {
  await refreshGamePathStatus();
  if (gamePathStatus.value.path) {
    path.value = gamePathStatus.value.path;
  }
});
</script>

<template>
  <form class="game-path-form" @submit.prevent="submitPath">
    <label :for="inputId">Game directory</label>
    <div class="path-row">
      <input
        :id="inputId"
        v-model="path"
        type="text"
        placeholder="/path/to/Zeepkist"
        :disabled="loading"
      />
      <button
        type="button"
        class="btn-secondary browse-button"
        :disabled="loading"
        @click="browseFolder"
      >
        Browse…
      </button>
    </div>
    <button type="submit" :disabled="loading">
      {{ loading ? "Saving…" : submitLabel }}
    </button>

    <p v-if="gamePathStatus.message && !error" class="hint status-hint">
      {{ gamePathStatus.message }}
    </p>
    <p v-if="error" class="error">{{ error }}</p>
  </form>
</template>

<style scoped>
.game-path-form {
  display: flex;
  flex-direction: column;
  gap: 0.75rem;
}

label {
  font-size: 0.8rem;
  font-weight: 600;
  color: var(--modio-text-muted);
  text-transform: uppercase;
  letter-spacing: 0.04em;
}

.path-row {
  display: flex;
  gap: 0.75rem;
}

.path-row input {
  flex: 1;
  min-width: 0;
}

.browse-button {
  flex-shrink: 0;
}

.status-hint,
.error {
  margin: 0;
}
</style>
