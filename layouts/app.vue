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
      <slot />
    </div>
  </div>
</template>
