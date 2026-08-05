<script setup lang="ts">
const { launchOverlayActive } = useGameLaunch();
</script>

<template>
  <Transition name="game-launch-overlay">
    <div
      v-if="launchOverlayActive"
      class="game-launch-overlay"
      aria-live="polite"
      aria-busy="true"
    >
      <div class="game-launch-overlay-card">
        <span class="game-launch-spinner" aria-hidden="true" />
        <p class="game-launch-overlay-message">Starting Zeepkist…</p>
      </div>
    </div>
  </Transition>
</template>

<style scoped>
.game-launch-overlay {
  position: fixed;
  inset: 0;
  z-index: 100;
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 1.5rem;
  background: rgba(18, 18, 20, 0.72);
  backdrop-filter: blur(4px);
}

.game-launch-overlay-card {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 0.85rem;
  max-width: 18rem;
  padding: 1.35rem 1.5rem;
  border-radius: var(--modio-radius);
  border: 1px solid var(--modio-border);
  background: rgba(26, 26, 27, 0.98);
  box-shadow: var(--modio-shadow);
  text-align: center;
}

.game-launch-overlay-message {
  margin: 0;
  font-size: 0.9rem;
  font-weight: 500;
  line-height: 1.4;
  color: var(--modio-text);
}

.game-launch-spinner {
  width: 1.35rem;
  height: 1.35rem;
  border: 2px solid var(--modio-border);
  border-top-color: var(--modio-accent);
  border-radius: 50%;
  animation: game-launch-spin 0.7s linear infinite;
}

.game-launch-overlay-enter-active,
.game-launch-overlay-leave-active {
  transition: opacity 0.18s ease;
}

.game-launch-overlay-enter-from,
.game-launch-overlay-leave-to {
  opacity: 0;
}

@keyframes game-launch-spin {
  to {
    transform: rotate(360deg);
  }
}
</style>
