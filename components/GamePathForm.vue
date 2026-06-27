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
  }>(),
  {
    submitLabel: "Save",
    inputId: "game-path",
  },
);

const emit = defineEmits<{
  saved: [];
}>();

const { gamePathStatus, refreshGamePathStatus, setGamePath, detectGamePaths, openGameFolder } =
  useGamePath();

const path = ref("");
const loading = ref(false);
const detecting = ref(false);
const openingFolder = ref(false);
const error = ref("");
const hint = ref("");
const candidates = ref<GamePathCandidate[]>([]);
const showManualEntry = ref(false);

const busy = computed(() => loading.value || detecting.value || openingFolder.value);
const showPathField = computed(
  () => showManualEntry.value || candidates.value.length > 0 || !!path.value,
);

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

  hint.value = "No Zeepkist install found. Try browsing to your game folder manually.";
}

async function runDetect() {
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

async function browseManually() {
  error.value = "";
  showManualEntry.value = true;
  candidates.value = [];
  hint.value = "";

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

async function openFolder() {
  openingFolder.value = true;
  error.value = "";

  try {
    await openGameFolder();
  } catch (err) {
    error.value = err instanceof Error ? err.message : String(err);
  } finally {
    openingFolder.value = false;
  }
}

onMounted(async () => {
  await refreshGamePathStatus();
  if (gamePathStatus.value.path) {
    path.value = gamePathStatus.value.path;
    showManualEntry.value = true;
  }
});
</script>

<template>
  <form class="game-path-form" @submit.prevent="submitPath">
    <div v-if="!showPathField" class="choice-actions">
      <button type="button" :disabled="busy" @click="runDetect">
        {{ detecting ? "Detecting…" : "Detect Zeepkist installation…" }}
      </button>
      <button
        type="button"
        class="btn-secondary"
        :disabled="busy"
        @click="browseManually"
      >
        Browse path manually
      </button>
    </div>

    <template v-else>
      <label :for="inputId">Game directory</label>
      <div class="path-row">
        <input
          :id="inputId"
          v-model="path"
          type="text"
          placeholder="/path/to/Zeepkist"
          :disabled="busy"
        />
        <button
          type="button"
          class="btn-secondary browse-button"
          :disabled="busy"
          @click="browseManually"
        >
          Browse…
        </button>
      </div>

      <div v-if="candidates.length > 1" class="candidate-list">
        <p class="hint candidate-label">Detected installs</p>
        <button
          v-for="candidate in candidates"
          :key="candidate.path"
          type="button"
          class="candidate-button"
          :disabled="busy"
          @click="applyCandidate(candidate)"
        >
          <span class="candidate-path">{{ candidate.path }}</span>
          <span class="candidate-source">{{ candidate.source }}</span>
        </button>
      </div>

      <button type="submit" :disabled="busy">
        {{ loading ? "Saving…" : submitLabel }}
      </button>

      <button
        v-if="!showManualEntry && candidates.length > 0"
        type="button"
        class="btn-secondary detect-again-button"
        :disabled="busy"
        @click="runDetect"
      >
        {{ detecting ? "Detecting…" : "Detect again" }}
      </button>
    </template>

    <p v-if="hint && !error" class="hint status-hint">{{ hint }}</p>
    <p v-else-if="gamePathStatus.message && !error && showPathField" class="hint status-hint">
      {{ gamePathStatus.message }}
    </p>

    <button
      v-if="gamePathStatus.valid && gamePathStatus.path"
      type="button"
      class="btn-secondary open-folder-button"
      :disabled="busy"
      @click="openFolder"
    >
      {{ openingFolder ? "Opening…" : "Open game folder" }}
    </button>

    <p v-if="error" class="error">{{ error }}</p>
  </form>
</template>

<style scoped>
.game-path-form {
  display: flex;
  flex-direction: column;
  gap: 0.75rem;
}

.choice-actions {
  display: flex;
  flex-direction: column;
  gap: 0.75rem;
}

.choice-actions button {
  width: 100%;
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

.detect-again-button,
.open-folder-button {
  width: 100%;
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
