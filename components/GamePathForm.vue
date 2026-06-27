<script setup lang="ts">
import { onMounted, ref } from "vue";
import { open } from "@tauri-apps/plugin-dialog";
import {
  type GamePathCandidate,
  useGamePath,
} from "~/composables/useGamePath";

const props = withDefaults(
  defineProps<{
    submitLabel?: string;
    inputId?: string;
    autoDetectOnMount?: boolean;
  }>(),
  {
    submitLabel: "Save",
    inputId: "game-path",
    autoDetectOnMount: false,
  },
);

const emit = defineEmits<{
  saved: [];
}>();

const { gamePathStatus, refreshGamePathStatus, setGamePath, detectGamePaths } =
  useGamePath();

const path = ref("");
const loading = ref(false);
const detecting = ref(false);
const error = ref("");
const hint = ref("");
const candidates = ref<GamePathCandidate[]>([]);

function applyCandidate(candidate: GamePathCandidate) {
  path.value = candidate.path;
  hint.value = `Found via ${candidate.source}.`;
}

function applyDetectionResults(found: GamePathCandidate[]) {
  candidates.value = found;

  if (found.length === 1) {
    applyCandidate(found[0]!);
    return;
  }

  if (found.length > 1) {
    hint.value = `Found ${found.length} installs. Pick one below or browse manually.`;
    return;
  }

  hint.value = "No Zeepkist install found. Browse to your game folder.";
}

async function runAutoDetect() {
  detecting.value = true;
  error.value = "";

  try {
    applyDetectionResults(await detectGamePaths());
  } catch (err) {
    error.value = String(err);
  } finally {
    detecting.value = false;
  }
}

async function browseFolder() {
  error.value = "";
  const selected = await open({
    directory: true,
    multiple: false,
    title: "Select Zeepkist game directory",
  });

  if (typeof selected === "string") {
    path.value = selected;
    candidates.value = [];
    hint.value = "";
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
    return;
  }

  if (props.autoDetectOnMount) {
    await runAutoDetect();
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
        :disabled="loading || detecting"
      />
      <button
        type="button"
        class="btn-secondary browse-button"
        :disabled="loading || detecting"
        @click="browseFolder"
      >
        Browse…
      </button>
    </div>

    <div class="action-row">
      <button type="submit" :disabled="loading || detecting">
        {{ loading ? "Saving…" : submitLabel }}
      </button>
      <button
        type="button"
        class="btn-secondary"
        :disabled="loading || detecting"
        @click="runAutoDetect"
      >
        {{ detecting ? "Detecting…" : "Auto-detect" }}
      </button>
    </div>

    <div v-if="candidates.length > 1" class="candidate-list">
      <p class="hint candidate-label">Detected installs</p>
      <button
        v-for="candidate in candidates"
        :key="candidate.path"
        type="button"
        class="candidate-button"
        :disabled="loading || detecting"
        @click="applyCandidate(candidate)"
      >
        <span class="candidate-path">{{ candidate.path }}</span>
        <span class="candidate-source">{{ candidate.source }}</span>
      </button>
    </div>

    <p v-if="hint && !error" class="hint status-hint">{{ hint }}</p>
    <p v-else-if="gamePathStatus.message && !error" class="hint status-hint">
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

.action-row {
  display: flex;
  flex-wrap: wrap;
  gap: 0.75rem;
}

.candidate-list {
  display: flex;
  flex-direction: column;
  gap: 0.5rem;
}

.candidate-label {
  margin: 0;
}

.candidate-button {
  display: flex;
  flex-direction: column;
  align-items: flex-start;
  gap: 0.15rem;
  width: 100%;
  padding: 0.65rem 0.75rem;
  border-radius: var(--modio-radius-sm, 0.375rem);
  border: 1px solid var(--modio-border);
  background: var(--modio-bg);
  color: var(--modio-text);
  text-align: left;
  cursor: pointer;
}

.candidate-button:hover:not(:disabled) {
  border-color: var(--modio-accent);
}

.candidate-path {
  font-size: 0.9rem;
  word-break: break-all;
}

.candidate-source {
  font-size: 0.75rem;
  color: var(--modio-text-muted);
}

.status-hint,
.error {
  margin: 0;
}
</style>
