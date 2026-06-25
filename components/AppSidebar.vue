<script setup lang="ts">
import { computed } from "vue";

const route = useRoute();
const { authStatus, refreshAuthStatus } = useModioAuth();
const { updateCount } = useModInstall();

onMounted(() => {
  refreshAuthStatus();
});

const primaryNav = computed(() => [
  { label: "Mods", to: "/home" },
  { label: "Installed", to: "/installed" },
  { label: "Updates", to: "/updates", badge: updateCount.value },
]);

const footerNav = computed(() => [
  { label: "Settings", to: "/settings" },
  authStatus.value.loggedIn
    ? { label: "User", to: "/user" }
    : { label: "Sign in", to: { path: "/", query: { redirect: "/user" } } },
]);

function isActive(path: string | { path: string }) {
  const target = typeof path === "string" ? path : path.path;
  if (target === "/home") {
    return route.path === "/home" || route.path.startsWith("/mods/");
  }
  return route.path === target;
}
</script>

<template>
  <aside class="app-sidebar" aria-label="Main navigation">
    <div class="app-sidebar-brand">
      <span class="app-sidebar-mark" aria-hidden="true" />
      <span class="app-sidebar-title">Modkist</span>
    </div>

    <nav class="app-sidebar-nav app-sidebar-nav--primary">
      <NuxtLink
        v-for="item in primaryNav"
        :key="item.to"
        :to="item.to"
        class="app-sidebar-link"
        :class="{ 'app-sidebar-link--active': isActive(item.to) }"
      >
        <span>{{ item.label }}</span>
        <span
          v-if="item.badge && item.badge > 0"
          class="app-sidebar-badge"
          :aria-label="`${item.badge} updates available`"
        >
          {{ item.badge }}
        </span>
      </NuxtLink>
    </nav>

    <nav class="app-sidebar-nav app-sidebar-nav--footer">
      <NuxtLink
        v-for="item in footerNav"
        :key="item.label"
        :to="item.to"
        class="app-sidebar-link"
        :class="{ 'app-sidebar-link--active': isActive(item.to) }"
      >
        {{ item.label }}
      </NuxtLink>
    </nav>
  </aside>
</template>

<style scoped>
.app-sidebar-link {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 0.5rem;
}

.app-sidebar-badge {
  min-width: 1.35rem;
  padding: 0.1rem 0.4rem;
  border-radius: 999px;
  background: rgba(7, 193, 216, 0.18);
  color: var(--modio-accent);
  font-size: 0.72rem;
  font-weight: 700;
  line-height: 1.2;
  text-align: center;
}
</style>
