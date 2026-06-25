<script setup lang="ts">
const { profileSwitchActive, profileSwitchMessage } = useProfileSwitchUi();
</script>

<template>
  <Transition name="profile-switch-overlay">
    <div
      v-if="profileSwitchActive"
      class="profile-switch-overlay"
      aria-live="polite"
      aria-busy="true"
    >
      <div class="profile-switch-overlay-card">
        <span class="profile-switch-spinner" aria-hidden="true" />
        <p class="profile-switch-overlay-message">
          {{ profileSwitchMessage }}
        </p>
      </div>
    </div>
  </Transition>
</template>

<style scoped>
.profile-switch-overlay {
  position: absolute;
  inset: 0;
  z-index: 30;
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 1.5rem;
  background: rgba(18, 18, 20, 0.72);
  backdrop-filter: blur(4px);
}

.profile-switch-overlay-card {
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

.profile-switch-overlay-message {
  margin: 0;
  font-size: 0.9rem;
  font-weight: 500;
  line-height: 1.4;
  color: var(--modio-text);
}

.profile-switch-spinner {
  width: 1.35rem;
  height: 1.35rem;
  border: 2px solid var(--modio-border);
  border-top-color: var(--modio-accent);
  border-radius: 50%;
  animation: profile-switch-spin 0.7s linear infinite;
}

.profile-switch-overlay-enter-active,
.profile-switch-overlay-leave-active {
  transition: opacity 0.18s ease;
}

.profile-switch-overlay-enter-from,
.profile-switch-overlay-leave-to {
  opacity: 0;
}

@keyframes profile-switch-spin {
  to {
    transform: rotate(360deg);
  }
}
</style>
