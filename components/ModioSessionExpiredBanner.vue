<script setup lang="ts">
const { authStatus } = useModioAuth();
const route = useRoute();

const dismissed = ref(false);

const visible = computed(
  () =>
    !dismissed.value &&
    !authStatus.value.loggedIn &&
    authStatus.value.sessionExpired === true,
);

function signIn() {
  navigateTo({ path: "/", query: { redirect: route.fullPath } });
}

// Show the banner again if a later session expires too.
watch(
  () => authStatus.value.loggedIn,
  (loggedIn) => {
    if (loggedIn) dismissed.value = false;
  },
);
</script>

<template>
  <aside
    v-if="visible"
    class="session-expired-banner"
    role="status"
    aria-live="polite"
  >
    <div class="session-expired-banner-body">
      <p class="session-expired-banner-title">Signed out of mod.io</p>
      <p class="session-expired-banner-text">
        Your mod.io session expired or was revoked. Sign in again to sync
        subscriptions and install private mods.
      </p>
    </div>
    <div class="session-expired-banner-actions">
      <button type="button" @click="signIn">Sign in</button>
      <button
        type="button"
        class="session-expired-banner-dismiss"
        aria-label="Dismiss signed out notice"
        @click="dismissed = true"
      >
        ×
      </button>
    </div>
  </aside>
</template>

<style scoped>
.session-expired-banner {
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

.session-expired-banner-body {
  flex: 1 1 16rem;
  min-width: 0;
}

.session-expired-banner-title {
  margin: 0 0 0.35rem;
  font-size: 0.95rem;
  font-weight: 650;
  color: #fbbf24;
}

.session-expired-banner-text {
  margin: 0;
  font-size: 0.875rem;
  line-height: 1.45;
  color: var(--modio-text);
}

.session-expired-banner-actions {
  display: flex;
  align-items: center;
  gap: 0.65rem;
  flex-shrink: 0;
}

.session-expired-banner-dismiss {
  padding: 0.15rem 0.45rem;
  border: none;
  background: transparent;
  color: var(--modio-text-muted);
  font-size: 1.25rem;
  line-height: 1;
  cursor: pointer;
}

.session-expired-banner-dismiss:hover {
  color: var(--modio-text);
  background: transparent;
}
</style>
