<script setup lang="ts">
import { onMounted, ref } from "vue";
import { open } from "@tauri-apps/plugin-dialog";
import { useGamePath } from "~/composables/useGamePath";

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
      await navigateTo("/home");
    }
  } catch (err) {
    error.value = String(err);
  } finally {
    loading.value = false;
  }
}

onMounted(async () => {
  await refreshGamePathStatus();
  if (gamePathStatus.value.valid) {
    await navigateTo("/home");
    return;
  }

  if (gamePathStatus.value.path) {
    path.value = gamePathStatus.value.path;
  }
});
</script>

<template>
  <main class="setup-shell">
    <div class="setup">
      <div class="setup-brand">
        <span class="setup-brand-mark" aria-hidden="true" />
        <h1>Locate Zeepkist</h1>
      </div>

      <p class="hint setup-intro">
        Point Modkist to the folder that contains
        <code>zeepkist.exe</code>.
      </p>

      <section class="panel">
        <form class="form" @submit.prevent="submitPath">
          <label for="game-path">Game directory</label>
          <div class="path-row">
            <input
              id="game-path"
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
            {{ loading ? "Saving…" : "Continue" }}
          </button>
        </form>

        <p v-if="gamePathStatus.message && !error" class="hint status-hint">
          {{ gamePathStatus.message }}
        </p>
        <p v-if="error" class="error">{{ error }}</p>
      </section>
    </div>
  </main>
</template>

<style scoped>
.setup-shell {
  min-height: 100vh;
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 2rem 1.5rem;
  background:
    radial-gradient(
      ellipse 70% 45% at 50% -10%,
      rgba(7, 193, 216, 0.14),
      transparent
    ),
    var(--modio-bg);
}

.setup {
  width: 100%;
  max-width: 32rem;
}

.setup-brand {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 0.75rem;
  margin-bottom: 1rem;
}

.setup-brand-mark {
  width: 0.35rem;
  height: 1.75rem;
  border-radius: 999px;
  background: var(--modio-accent);
}

.setup-brand h1 {
  margin: 0;
  font-size: 1.5rem;
  font-weight: 700;
  letter-spacing: -0.02em;
}

.setup-intro {
  text-align: center;
  margin: 0 0 1.25rem;
}

.setup-intro code {
  color: var(--modio-text);
  font-size: 0.85em;
}

.panel {
  padding: 1.5rem;
  border-radius: var(--modio-radius);
  background: var(--modio-surface);
  border: 1px solid var(--modio-border);
  box-shadow: var(--modio-shadow);
}

.form {
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
  margin-top: 1rem;
}
</style>
