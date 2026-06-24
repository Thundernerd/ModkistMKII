<script setup lang="ts">
import { ensureBepInEx, ensureGamePath } from "~/utils/authNavigation";

const ready = ref(false);

onMounted(async () => {
  if (!(await ensureGamePath())) {
    return;
  }

  ready.value = await ensureBepInEx();
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
