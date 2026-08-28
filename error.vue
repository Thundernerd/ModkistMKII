<script setup lang="ts">
const error = useError();

const title = computed(() => {
  const status = Number(error.value?.statusCode ?? 500);
  if (status === 404) {
    return "Page not found";
  }
  return "Something went wrong";
});

const description = computed(() => {
  const message = error.value?.message?.trim();
  if (message) {
    return message;
  }
  return "An unexpected error occurred. You can continue using Modkist.";
});

async function continueUsingApp() {
  await clearError({ redirect: "/home" });
}
</script>

<template>
  <div class="app-error-page">
    <div class="app-error-card">
      <h1 class="app-error-title">{{ title }}</h1>
      <p class="app-error-message">{{ description }}</p>
      <button type="button" class="btn-primary app-error-continue" @click="continueUsingApp">
        Continue
      </button>
    </div>
  </div>
</template>

<style scoped>
.app-error-page {
  min-height: 100vh;
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 1.5rem;
  background: var(--modio-bg);
}

.app-error-card {
  width: min(100%, 28rem);
  padding: 1.75rem 1.85rem;
  border-radius: var(--modio-radius);
  border: 1px solid var(--modio-border);
  background: var(--modio-surface);
  box-shadow: var(--modio-shadow);
  text-align: center;
}

.app-error-title {
  margin: 0 0 0.75rem;
  font-size: 1.35rem;
  font-weight: 700;
  color: var(--modio-text);
}

.app-error-message {
  margin: 0 0 1.25rem;
  font-size: 0.92rem;
  line-height: 1.5;
  color: var(--modio-text-muted);
  word-break: break-word;
}

.app-error-continue {
  min-width: 8rem;
}
</style>
