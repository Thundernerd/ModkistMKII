<script setup lang="ts">
import { useNotifications } from "~/composables/useNotifications";

const { notifications, dismissNotification } = useNotifications();
</script>

<template>
  <div
    class="app-notifications"
    aria-live="polite"
    aria-relevant="additions text"
  >
    <TransitionGroup name="app-notification">
      <article
        v-for="notification in notifications"
        :key="notification.id"
        class="app-notification"
        :class="`app-notification--${notification.tone}`"
        role="status"
      >
        <div class="app-notification-body">
          <h2 class="app-notification-title">{{ notification.title }}</h2>
          <p class="app-notification-message">{{ notification.message }}</p>
        </div>
        <button
          type="button"
          class="app-notification-dismiss"
          aria-label="Dismiss notification"
          @click="dismissNotification(notification.id)"
        >
          ×
        </button>
      </article>
    </TransitionGroup>
  </div>
</template>

<style scoped>
.app-notifications {
  position: fixed;
  top: 1rem;
  right: 1rem;
  z-index: 120;
  display: flex;
  flex-direction: column-reverse;
  gap: 0.65rem;
  width: min(22rem, calc(100vw - 2rem));
  pointer-events: none;
}

.app-notification {
  display: flex;
  align-items: flex-start;
  gap: 0.75rem;
  padding: 0.9rem 0.95rem;
  border-radius: var(--modio-radius);
  border: 1px solid var(--modio-border);
  background: var(--modio-surface-raised);
  box-shadow: 0 12px 32px rgba(0, 0, 0, 0.28);
  pointer-events: auto;
}

.app-notification--error {
  border-color: #c45c5c;
  background: #2b1f1f;
}

.app-notification--success {
  border-color: #3f9f66;
  background: #1a2820;
}

.app-notification--warning {
  border-color: #c9a227;
  background: #2b2618;
}

.app-notification--info {
  border-color: var(--modio-border);
  background: var(--modio-surface-raised);
}

.app-notification-body {
  flex: 1;
  min-width: 0;
}

.app-notification-title {
  margin: 0 0 0.25rem;
  font-size: 0.9rem;
  font-weight: 650;
  color: var(--modio-text);
}

.app-notification--error .app-notification-title {
  color: var(--modio-danger);
}

.app-notification--success .app-notification-title {
  color: var(--modio-success);
}

.app-notification--warning .app-notification-title {
  color: #e6c547;
}

.app-notification-message {
  margin: 0;
  font-size: 0.84rem;
  line-height: 1.45;
  color: var(--modio-text-muted);
}

.app-notification-dismiss {
  flex: 0 0 auto;
  width: 1.6rem;
  height: 1.6rem;
  margin: -0.15rem -0.1rem 0 0;
  border: none;
  border-radius: var(--modio-radius-sm);
  background: transparent;
  color: var(--modio-text-muted);
  font-size: 1.2rem;
  line-height: 1;
}

.app-notification-dismiss:hover {
  color: var(--modio-text);
  background: var(--modio-surface-hover);
}

.app-notification-enter-active,
.app-notification-leave-active {
  transition:
    transform 0.28s ease,
    opacity 0.28s ease;
}

.app-notification-enter-from,
.app-notification-leave-to {
  transform: translateX(calc(100% + 1rem));
  opacity: 0;
}

.app-notification-move {
  transition: transform 0.28s ease;
}
</style>
