<script setup lang="ts">
import { ensureGamePath } from "~/utils/authNavigation";

const ready = ref(false);

onMounted(async () => {
  ready.value = await ensureGamePath();
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
