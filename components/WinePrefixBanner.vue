<script setup lang="ts">
const {
  wineFeedback,
  wineChecking,
  wineError,
  needsWineAttention,
  configureWineWinhttp,
} = useWineWinhttp();

async function retry() {
  try {
    await configureWineWinhttp();
  } catch {
    // Error stays in wineError for display.
  }
}
</script>

<template>
  <aside
    v-if="needsWineAttention"
    class="wine-banner"
    role="status"
    aria-live="polite"
  >
    <div class="wine-banner-body">
      <p class="wine-banner-title">Wine prefix not found</p>
      <p class="wine-banner-text">
        {{
          wineFeedback?.text ||
          wineError ||
          "Did not find a Wine prefix. Launch Zeepkist one time. Then retry."
        }}
      </p>
    </div>
    <div class="wine-banner-actions">
      <button
        type="button"
        class="btn-secondary wine-banner-retry"
        :disabled="wineChecking"
        @click="retry"
      >
        {{ wineChecking ? "Wait…" : "Retry" }}
      </button>
      <NuxtLink to="/settings" class="wine-banner-settings">
        Settings
      </NuxtLink>
    </div>
  </aside>
</template>

<style scoped>
.wine-banner {
  display: flex;
  flex-wrap: wrap;
  align-items: flex-start;
  justify-content: space-between;
  gap: 0.85rem 1rem;
  margin: 0 0 1.25rem;
  padding: 0.9rem 1rem;
  border-radius: var(--modio-radius);
  border: 1px solid rgba(251, 191, 36, 0.45);
  background: rgba(251, 191, 36, 0.1);
}

.wine-banner-body {
  flex: 1 1 16rem;
  min-width: 0;
}

.wine-banner-title {
  margin: 0 0 0.35rem;
  font-size: 0.95rem;
  font-weight: 650;
  color: #fbbf24;
}

.wine-banner-text {
  margin: 0;
  font-size: 0.875rem;
  line-height: 1.45;
  color: var(--modio-text);
}

.wine-banner-actions {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 0.65rem;
  flex-shrink: 0;
}

.wine-banner-retry {
  min-width: 6.5rem;
}

.wine-banner-settings {
  font-size: 0.875rem;
  color: var(--modio-accent);
  text-decoration: none;
}

.wine-banner-settings:hover {
  color: var(--modio-accent-hover);
  text-decoration: underline;
}
</style>
