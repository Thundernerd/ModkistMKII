<script setup lang="ts">
import {
  continuePastBepInEx,
  navigateToApp,
  readRedirectParam,
} from "~/utils/authNavigation";
import { wineWinhttpFeedback } from "~/utils/wineWinhttp";
import { BEPINEX_REQUIRED_VERSION } from "~/composables/useBepInEx";

const route = useRoute();
const {
  bepinexStatus,
  loading,
  installing,
  error,
  refreshBepInExStatus,
  installBepInEx,
} = useBepInEx();

const redirect = computed(() => readRedirectParam(route.query.redirect));
const phase = ref<"checking" | "installing" | "warning" | "wineWarning" | "error">(
  "checking",
);

const wineFeedback = computed(() =>
  wineWinhttpFeedback(bepinexStatus.value.wineWinhttp),
);

async function runInstall() {
  phase.value = "installing";
  error.value = "";

  try {
    await installBepInEx();
    const feedback = wineWinhttpFeedback(bepinexStatus.value.wineWinhttp);
    if (feedback && feedback.tone !== "success") {
      phase.value = "wineWarning";
      return;
    }
    await navigateToApp(redirect.value);
  } catch {
    phase.value = "error";
  }
}

async function handleStatus() {
  const status = bepinexStatus.value;

  if (status.state === "installed") {
    await navigateToApp(redirect.value);
    return;
  }

  if (status.state === "wrongVersion") {
    phase.value = "warning";
    return;
  }

  await runInstall();
}

async function continueAnyway() {
  await continuePastBepInEx(redirect.value);
}

async function continueAfterWineWarning() {
  await navigateToApp(redirect.value);
}

async function retry() {
  await refreshBepInExStatus();
  if (error.value) {
    phase.value = "error";
    return;
  }

  await handleStatus();
}

onMounted(async () => {
  await refreshBepInExStatus();
  if (error.value) {
    phase.value = "error";
    return;
  }

  await handleStatus();
});
</script>

<template>
  <main class="setup-shell">
    <div class="setup">
      <div class="setup-brand">
        <span class="setup-brand-mark" aria-hidden="true" />
        <h1>Install BepInEx</h1>
      </div>

      <p class="hint setup-intro">
        Modkist needs BepInEx {{ BEPINEX_REQUIRED_VERSION }} (x64) in your game folder to load mods.
      </p>

      <section class="panel">
        <div v-if="phase === 'checking' || loading" class="state">
          <span class="spinner" aria-hidden="true" />
          Checking BepInEx installation…
        </div>

        <div v-else-if="phase === 'installing' || installing" class="state">
          <span class="spinner" aria-hidden="true" />
          Downloading and installing BepInEx {{ BEPINEX_REQUIRED_VERSION }}…
        </div>

        <div v-else-if="phase === 'warning'" class="warning">
          <p class="warning-text">
            {{
              bepinexStatus.message ||
              "A different BepInEx version was found in your game directory."
            }}
          </p>
          <p v-if="bepinexStatus.foundVersion" class="meta">
            Detected version: {{ bepinexStatus.foundVersion }}
          </p>
          <button type="button" class="continue-button" @click="continueAnyway">
            Continue anyway
          </button>
        </div>

        <div v-else-if="phase === 'wineWarning'" class="warning">
          <p class="warning-text">
            BepInEx was installed, but Modkist could not fully configure your
            Wine prefix for mod loading.
          </p>
          <p
            v-if="wineFeedback"
            :class="wineFeedback.tone === 'error' ? 'error' : 'meta'"
          >
            {{ wineFeedback.text }}
          </p>
          <button type="button" class="continue-button" @click="continueAfterWineWarning">
            Continue anyway
          </button>
        </div>

        <div v-else class="error-state">
          <p class="error">{{ error || "Could not verify BepInEx." }}</p>
          <button type="button" class="retry-button" @click="retry">
            Try again
          </button>
        </div>
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
      rgba(var(--modio-accent-rgb), 0.14),
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

.panel {
  padding: 1.5rem;
  border-radius: var(--modio-radius);
  background: var(--modio-surface);
  border: 1px solid var(--modio-border);
  box-shadow: var(--modio-shadow);
}

.state {
  display: flex;
  align-items: center;
  gap: 0.75rem;
  color: var(--modio-text-muted);
}

.spinner {
  width: 1.1rem;
  height: 1.1rem;
  border: 2px solid var(--modio-border);
  border-top-color: var(--modio-accent);
  border-radius: 50%;
  animation: spin 0.7s linear infinite;
  flex-shrink: 0;
}

.warning-text {
  margin: 0 0 0.75rem;
  line-height: 1.5;
  color: var(--modio-text);
}

.meta {
  margin: 0 0 1rem;
}

.continue-button,
.retry-button {
  width: 100%;
}

.error-state .error {
  margin: 0 0 1rem;
}

@keyframes spin {
  to {
    transform: rotate(360deg);
  }
}
</style>
