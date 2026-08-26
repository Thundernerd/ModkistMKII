<script setup lang="ts">
const { gatePassed, runUpdateGate } = useAppUpdater();

onMounted(() => {
  runUpdateGate();
});
</script>

<template>
  <AppUpdateGate v-if="!gatePassed" />
  <NuxtLayout v-else>
    <NuxtPage />
  </NuxtLayout>
</template>
