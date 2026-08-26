<script setup lang="ts">
const {
  phase,
  version,
  downloaded,
  total,
  relaunchFailed,
  restartNow,
} = useAppUpdater();

const title = computed(() => {
  switch (phase.value) {
    case "checking":
      return "Checking for updates";
    case "downloading":
      return "Update found";
    case "restarting":
      return "Update installed";
  }
});

const message = computed(() => {
  switch (phase.value) {
    case "checking":
      return "Checking for updates…";
    case "downloading":
      return version.value
        ? `Downloading Modkist v${version.value}…`
        : "Downloading update…";
    case "restarting":
      return relaunchFailed.value
        ? "The update is installed, but Modkist could not restart automatically."
        : "Update installed. Restarting…";
  }
});

const progressPercent = computed(() => {
  if (!total.value || total.value <= 0) return null;
  return Math.min(100, Math.round((downloaded.value / total.value) * 100));
});

const busy = computed(
  () => phase.value !== "restarting" || !relaunchFailed.value,
);
</script>

<template>
  <div
    class="app-update-gate"
    role="dialog"
    aria-modal="true"
    aria-labelledby="app-update-gate-title"
    :aria-busy="busy"
  >
    <div class="app-update-gate-card">
      <span
        v-if="busy"
        class="app-update-gate-spinner"
        aria-hidden="true"
      />
      <h1 id="app-update-gate-title" class="app-update-gate-title">
        {{ title }}
      </h1>
      <p class="app-update-gate-message" aria-live="polite">{{ message }}</p>
      <div
        v-if="phase === 'downloading' && progressPercent != null"
        class="app-update-gate-progress"
      >
        <div
          class="app-update-gate-progress-bar"
          role="progressbar"
          :aria-valuenow="progressPercent"
          aria-valuemin="0"
          aria-valuemax="100"
        >
          <div
            class="app-update-gate-progress-fill"
            :style="{ width: `${progressPercent}%` }"
          />
        </div>
        <p class="app-update-gate-progress-label">{{ progressPercent }}%</p>
      </div>
      <button
        v-if="phase === 'restarting' && relaunchFailed"
        type="button"
        class="app-update-gate-restart"
        @click="restartNow"
      >
        Restart
      </button>
    </div>
  </div>
</template>

<style scoped>
.app-update-gate {
  position: fixed;
  inset: 0;
  z-index: 200;
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 1.5rem;
  background: var(--modio-bg);
}

.app-update-gate-card {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 0.75rem;
  width: min(100%, 22rem);
  padding: 1.5rem 1.6rem 1.35rem;
  border-radius: var(--modio-radius);
  border: 1px solid var(--modio-border);
  background: var(--modio-surface);
  box-shadow: var(--modio-shadow);
  text-align: center;
}

.app-update-gate-title {
  margin: 0;
  font-size: 1.15rem;
  font-weight: 600;
  color: var(--modio-text);
}

.app-update-gate-message {
  margin: 0;
  font-size: 0.9rem;
  font-weight: 500;
  line-height: 1.45;
  color: var(--modio-text-muted);
}

.app-update-gate-spinner {
  width: 1.5rem;
  height: 1.5rem;
  border: 2px solid var(--modio-border);
  border-top-color: var(--modio-accent);
  border-radius: 50%;
  animation: app-update-gate-spin 0.7s linear infinite;
}

.app-update-gate-progress {
  display: flex;
  flex-direction: column;
  gap: 0.4rem;
  width: 100%;
  margin-top: 0.25rem;
}

.app-update-gate-progress-bar {
  width: 100%;
  height: 0.45rem;
  overflow: hidden;
  border-radius: 999px;
  background: var(--modio-surface-raised);
  border: 1px solid var(--modio-border);
}

.app-update-gate-progress-fill {
  height: 100%;
  background: var(--modio-accent);
  transition: width 0.15s ease;
}

.app-update-gate-progress-label {
  margin: 0;
  font-size: 0.8rem;
  color: var(--modio-text-muted);
}

.app-update-gate-restart {
  margin-top: 0.35rem;
}

@keyframes app-update-gate-spin {
  to {
    transform: rotate(360deg);
  }
}
</style>
