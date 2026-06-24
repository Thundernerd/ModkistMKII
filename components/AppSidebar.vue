<script setup lang="ts">
import { computed } from "vue";

const route = useRoute();
const { authStatus, refreshAuthStatus } = useModioAuth();

onMounted(() => {
  refreshAuthStatus();
});

const primaryNav = [{ label: "Mods", to: "/home" }];

const footerNav = computed(() => [
  { label: "Settings", to: "/settings" },
  authStatus.value.loggedIn
    ? { label: "User", to: "/user" }
    : { label: "Sign in", to: { path: "/", query: { redirect: "/user" } } },
]);

function isActive(path: string | { path: string }) {
  const target = typeof path === "string" ? path : path.path;
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
        {{ item.label }}
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
