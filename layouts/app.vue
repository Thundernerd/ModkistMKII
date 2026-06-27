<script setup lang="ts">
import { ensureBepInEx, ensureGamePath } from "~/utils/authNavigation";

const ready = ref(false);
const { startGameProcessPolling, stopGameProcessPolling } = useGameProcess();

onMounted(async () => {
  if (!(await ensureGamePath())) {
    return;
  }

  ready.value = await ensureBepInEx();
  if (ready.value) {
    startGameProcessPolling();
  }
});

onUnmounted(() => {
  stopGameProcessPolling();
});
</script>

<template>
  <div v-if="ready" class="app-shell">
    <AppSidebar />
    <div class="app-main">
      <ProfileSwitchOverlay />
      <slot />
    </div>
  </div>
  <div v-else class="app-loading" aria-live="polite" aria-busy="true">
    <span class="app-loading-spinner" aria-hidden="true" />
    <p>Loading Modkist…</p>
  </div>
</template>

<style scoped>
.app-loading {
  min-height: 100vh;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 0.85rem;
  color: var(--modio-text-muted);
  background: var(--modio-bg);
}

.app-loading p {
  margin: 0;
  font-size: 0.95rem;
}

.app-loading-spinner {
  width: 1.35rem;
  height: 1.35rem;
  border: 2px solid var(--modio-border);
  border-top-color: var(--modio-accent);
  border-radius: 50%;
  animation: app-loading-spin 0.7s linear infinite;
}

@keyframes app-loading-spin {
  to {
    transform: rotate(360deg);
  }
}
</style>
