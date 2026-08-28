<script setup lang="ts">
import { useModioRateLimit } from "~/composables/useModioRateLimit";

const {
  rateLimitBannerVisible,
  rateLimitBannerMessage,
  dismissRateLimitBanner,
} = useModioRateLimit();
</script>

<template>
  <div
    v-if="rateLimitBannerVisible"
    class="modio-rate-limit-banner"
    role="status"
    aria-live="polite"
  >
    <p class="modio-rate-limit-banner-message">{{ rateLimitBannerMessage }}</p>
    <button
      type="button"
      class="modio-rate-limit-banner-dismiss"
      aria-label="Dismiss rate limit notice"
      @click="dismissRateLimitBanner"
    >
      ×
    </button>
  </div>
</template>

<style scoped>
.modio-rate-limit-banner {
  display: flex;
  align-items: flex-start;
  gap: 0.75rem;
  margin: 0 0 1rem;
  padding: 0.85rem 1rem;
  border-radius: var(--modio-radius-sm);
  border: 1px solid rgba(255, 180, 80, 0.45);
  background: rgba(255, 180, 80, 0.1);
  color: var(--modio-text);
}

.modio-rate-limit-banner-message {
  flex: 1;
  margin: 0;
  font-size: 0.9rem;
  line-height: 1.45;
}

.modio-rate-limit-banner-dismiss {
  flex-shrink: 0;
  margin: -0.15rem -0.25rem 0 0;
  padding: 0.15rem 0.45rem;
  border: none;
  background: transparent;
  color: var(--modio-text-muted);
  font-size: 1.25rem;
  line-height: 1;
  cursor: pointer;
}

.modio-rate-limit-banner-dismiss:hover {
  color: var(--modio-text);
  background: transparent;
}
</style>
