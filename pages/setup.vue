<script setup lang="ts">
import {
  navigateToApp,
  readRedirectParam,
} from "~/utils/authNavigation";
import { useGamePath } from "~/composables/useGamePath";

const route = useRoute();
const { gamePathStatus, refreshGamePathStatus } = useGamePath();

const redirect = computed(() => readRedirectParam(route.query.redirect));

async function onPathSaved() {
  await navigateToApp(redirect.value);
}

onMounted(async () => {
  await refreshGamePathStatus();
  if (gamePathStatus.value.valid) {
    await navigateToApp(redirect.value);
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
        <GamePathForm
          input-id="setup-game-path"
          submit-label="Continue"
          @saved="onPathSaved"
        />
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
</style>
