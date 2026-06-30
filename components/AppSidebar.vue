<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from "vue";
import type { ProfileKind, ProfileSummary } from "~/composables/useProfiles";

const route = useRoute();
const { authStatus, refreshAuthStatus } = useModioAuth();
const {
  profiles,
  activeProfile,
  switching,
  loading: profilesLoading,
  refreshProfiles,
} = useProfiles();
const {
  updateCount,
  installingIds,
  uninstallingIds,
  syncingSubscriptions,
  bulkUpdating,
  checkingUpdates,
} = useModInstall();
const { gameRunning } = useGameProcess();
const { launching, launchError, launchGame, clearLaunchError } = useGameLaunch();
const { activateProfile } = useProfileActivation();
const {
  profileSwitchActive,
  profileSwitchMessage,
  profileSwitchTargetName,
} = useProfileSwitchUi();

const profileError = ref("");
const menuOpen = ref(false);
const switcherRoot = ref<HTMLElement | null>(null);

onMounted(async () => {
  await refreshAuthStatus();
  try {
    await refreshProfiles();
  } catch (error) {
    profileError.value =
      error instanceof Error ? error.message : String(error);
  }

  document.addEventListener("click", handleDocumentClick);
  document.addEventListener("keydown", handleDocumentKeydown);
});

onBeforeUnmount(() => {
  document.removeEventListener("click", handleDocumentClick);
  document.removeEventListener("keydown", handleDocumentKeydown);
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

const installBusy = computed(
  () =>
    installingIds.value.size > 0 ||
    uninstallingIds.value.size > 0 ||
    syncingSubscriptions.value ||
    bulkUpdating.value ||
    checkingUpdates.value,
);

const profileSelectDisabled = computed(
  () =>
    profileSwitchActive.value ||
    switching.value ||
    profilesLoading.value ||
    installBusy.value,
);

const profileTriggerBusy = computed(
  () => profileSwitchActive.value || switching.value || profilesLoading.value,
);

const profileTriggerName = computed(() => {
  if (profileSwitchActive.value) {
    return profileSwitchTargetName.value ?? "Switching…";
  }
  return activeProfile.value?.name ?? "Loading…";
});

const profileTriggerMeta = computed(() => {
  if (profileSwitchActive.value) {
    return { label: profileSwitchPhaseLabel(), tone: "accent" as const };
  }
  return profileKindMeta(activeProfile.value?.kind);
});

function profileSwitchPhaseLabel() {
  const text = profileSwitchMessage.value.toLowerCase();
  if (text.includes("syncing")) return "Syncing subscriptions";
  if (text.includes("loading")) return "Loading mods";
  return "Switching profile";
}

const statusMessage = computed(() => {
  if (profileSwitchActive.value) return profileSwitchMessage.value;
  if (switching.value) return "Switching profile…";
  if (profileError.value) return profileError.value;
  if (activeProfile.value?.installBlocked) {
    return "Installs disabled on this profile.";
  }
  return "";
});

const statusTone = computed(() => {
  if (profileError.value) return "error";
  if (profileSwitchActive.value || switching.value) return "loading";
  return "hint";
});

function profileKindMeta(kind?: ProfileKind) {
  switch (kind) {
    case "vanilla":
      return { label: "No mods", tone: "muted" as const };
    case "user":
      return { label: "Account", tone: "accent" as const };
    case "custom":
      return { label: "Custom", tone: "neutral" as const };
    default:
      return { label: "Profile", tone: "neutral" as const };
  }
}

function isActive(path: string | { path: string }) {
  const target = typeof path === "string" ? path : path.path;
  if (target === "/home") {
    return route.path === "/home" || route.path.startsWith("/mods/");
  }
  return route.path === target;
}

function handleDocumentClick(event: MouseEvent) {
  if (!menuOpen.value) return;
  const root = switcherRoot.value;
  if (root && !root.contains(event.target as Node)) {
    menuOpen.value = false;
  }
}

function handleDocumentKeydown(event: KeyboardEvent) {
  if (event.key === "Escape") {
    menuOpen.value = false;
  }
}

function toggleMenu() {
  if (profileSelectDisabled.value) return;
  menuOpen.value = !menuOpen.value;
}

async function handleLaunchGame() {
  clearLaunchError();
  try {
    await launchGame();
  } catch {
    // launchError is set in the composable
  }
}

async function selectProfile(profile: ProfileSummary) {
  if (!profile.selectable || profile.isActive || profileSelectDisabled.value) {
    return;
  }

  menuOpen.value = false;
  profileError.value = "";

  try {
    await activateProfile(profile);
  } catch (error) {
    profileError.value =
      error instanceof Error ? error.message : String(error);
  }
}
</script>

<template>
  <aside class="app-sidebar" aria-label="Main navigation">
    <div class="app-sidebar-brand">
      <span class="app-sidebar-mark" aria-hidden="true" />
      <span class="app-sidebar-title">Modkist</span>
    </div>

    <div ref="switcherRoot" class="profile-switcher">
      <span class="profile-switcher-label">Active profile</span>

      <button
        type="button"
        class="profile-trigger"
        :class="{
          'profile-trigger--open': menuOpen,
          'profile-trigger--busy': profileTriggerBusy,
        }"
        :disabled="profileSelectDisabled"
        :aria-expanded="menuOpen"
        aria-haspopup="listbox"
        @click="toggleMenu"
      >
        <span
          v-if="profileTriggerBusy"
          class="profile-trigger-spinner"
          aria-hidden="true"
        />
        <span
          v-else
          class="profile-trigger-icon"
          :class="`profile-trigger-icon--${activeProfile?.kind ?? 'custom'}`"
          aria-hidden="true"
        />
        <span class="profile-trigger-body">
          <span class="profile-trigger-name">
            {{ profileTriggerName }}
          </span>
          <span
            class="profile-trigger-meta"
            :class="`profile-trigger-meta--${profileTriggerMeta.tone}`"
          >
            {{ profileTriggerMeta.label }}
          </span>
        </span>
        <span class="profile-chevron" :class="{ 'profile-chevron--open': menuOpen }" />
      </button>

      <Transition name="profile-menu">
        <ul
          v-if="menuOpen"
          class="profile-menu"
          role="listbox"
          :aria-label="'Switch profile'"
        >
          <li
            v-for="profile in profiles"
            :key="profile.id"
            role="option"
            :aria-selected="profile.isActive"
          >
            <button
              type="button"
              class="profile-menu-item"
              :class="{
                'profile-menu-item--active': profile.isActive,
                'profile-menu-item--disabled': !profile.selectable,
              }"
              :disabled="!profile.selectable || profile.isActive || profileSelectDisabled"
              @click="selectProfile(profile)"
            >
              <span
                class="profile-menu-icon"
                :class="`profile-menu-icon--${profile.kind}`"
                aria-hidden="true"
              />
              <span class="profile-menu-copy">
                <span class="profile-menu-name">{{ profile.name }}</span>
                <span class="profile-menu-meta">
                  {{ profileKindMeta(profile.kind).label }}
                  <template v-if="!profile.selectable"> · Sign in required</template>
                  <template v-else-if="profile.installBlocked"> · Installs off</template>
                </span>
              </span>
              <span v-if="profile.isActive" class="profile-menu-check" aria-hidden="true" />
            </button>
          </li>

          <li class="profile-menu-footer">
            <NuxtLink to="/settings" class="profile-menu-manage" @click="menuOpen = false">
              Manage profiles
            </NuxtLink>
          </li>
        </ul>
      </Transition>

      <p
        v-if="statusMessage"
        class="profile-switcher-status"
        :class="`profile-switcher-status--${statusTone}`"
      >
        <span v-if="statusTone === 'loading'" class="profile-status-spinner" aria-hidden="true" />
        {{ statusMessage }}
      </p>
    </div>

    <div class="sidebar-play">
      <button
        type="button"
        class="sidebar-play-btn"
        :disabled="gameRunning || launching || installBusy"
        @click="handleLaunchGame"
      >
        <span class="sidebar-play-icon" aria-hidden="true" />
        <span>{{ gameRunning ? "Running" : launching ? "Launching…" : "Play" }}</span>
      </button>
      <p v-if="launchError" class="sidebar-play-error">{{ launchError }}</p>
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

.profile-switcher {
  position: relative;
  margin: 0 0.35rem 0.85rem;
  padding-bottom: 0.85rem;
  border-bottom: 1px solid var(--modio-border);
}

.profile-switcher-label {
  display: block;
  margin: 0 0.4rem 0.45rem;
  font-size: 0.68rem;
  font-weight: 600;
  letter-spacing: 0.06em;
  text-transform: uppercase;
  color: var(--modio-text-subtle);
}

.profile-trigger {
  display: flex;
  align-items: center;
  gap: 0.65rem;
  width: 100%;
  padding: 0.6rem 0.7rem;
  border-radius: var(--modio-radius);
  border: 1px solid var(--modio-border);
  background:
    linear-gradient(180deg, rgba(255, 255, 255, 0.03), transparent),
    var(--modio-surface-raised);
  color: var(--modio-text);
  text-align: left;
  box-shadow: inset 0 1px 0 rgba(255, 255, 255, 0.03);
  transition:
    border-color 0.2s ease,
    background-color 0.2s ease,
    box-shadow 0.2s ease;
}

.profile-trigger:hover:not(:disabled) {
  border-color: rgba(var(--modio-accent-rgb), 0.35);
  background:
    linear-gradient(180deg, rgba(var(--modio-accent-rgb), 0.06), transparent),
    var(--modio-surface-hover);
}

.profile-trigger--open {
  border-color: rgba(var(--modio-accent-rgb), 0.55);
  box-shadow:
    0 0 0 1px rgba(var(--modio-accent-rgb), 0.15),
    inset 0 1px 0 rgba(255, 255, 255, 0.04);
}

.profile-trigger--busy {
  opacity: 0.75;
}

.profile-trigger:disabled {
  opacity: 0.6;
  cursor: not-allowed;
}

.profile-trigger-icon,
.profile-menu-icon {
  position: relative;
  flex-shrink: 0;
  width: 1.85rem;
  height: 1.85rem;
  border-radius: 0.55rem;
  border: 1px solid var(--modio-border);
  background: var(--modio-surface);
}

.profile-trigger-spinner {
  flex-shrink: 0;
  width: 1.85rem;
  height: 1.85rem;
  border: 2px solid var(--modio-border);
  border-top-color: var(--modio-accent);
  border-radius: 50%;
  animation: profile-spin 0.7s linear infinite;
}

.profile-trigger-icon::after,
.profile-menu-icon::after {
  content: "";
  position: absolute;
  inset: 0;
  margin: auto;
  border-radius: 999px;
}

.profile-trigger-icon--vanilla::after,
.profile-menu-icon--vanilla::after {
  width: 0.55rem;
  height: 0.1rem;
  background: var(--modio-text-muted);
  border-radius: 1px;
}

.profile-trigger-icon--user::after,
.profile-menu-icon--user::after {
  width: 0.55rem;
  height: 0.55rem;
  top: 0.42rem;
  background: var(--modio-accent);
  box-shadow: 0 0.55rem 0 -0.18rem var(--modio-accent);
}

.profile-trigger-icon--custom::after,
.profile-menu-icon--custom::after {
  width: 0.7rem;
  height: 0.45rem;
  border: 1.5px solid var(--modio-text-muted);
  border-radius: 0.15rem;
  background: transparent;
  box-shadow: 0 -0.42rem 0 -0.05rem var(--modio-text-muted);
}

.profile-trigger-icon--user,
.profile-menu-icon--user {
  border-color: rgba(var(--modio-accent-rgb), 0.35);
  background: rgba(var(--modio-accent-rgb), 0.08);
}

.profile-trigger-body {
  display: flex;
  flex-direction: column;
  gap: 0.1rem;
  min-width: 0;
  flex: 1;
}

.profile-trigger-name,
.profile-menu-name {
  font-size: 0.86rem;
  font-weight: 600;
  line-height: 1.2;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.profile-trigger-meta,
.profile-menu-meta {
  font-size: 0.72rem;
  line-height: 1.2;
  color: var(--modio-text-muted);
}

.profile-trigger-meta--accent {
  color: var(--modio-accent);
}

.profile-trigger-meta--muted {
  color: var(--modio-text-subtle);
}

.profile-chevron {
  flex-shrink: 0;
  width: 0.45rem;
  height: 0.45rem;
  margin-right: 0.1rem;
  border-right: 1.5px solid var(--modio-text-muted);
  border-bottom: 1.5px solid var(--modio-text-muted);
  transform: rotate(45deg);
  transition: transform 0.2s ease;
}

.profile-chevron--open {
  transform: rotate(-135deg) translateY(1px);
}

.profile-menu {
  position: absolute;
  top: calc(100% + 0.35rem);
  left: 0;
  right: 0;
  z-index: 20;
  margin: 0;
  padding: 0.35rem;
  list-style: none;
  border-radius: var(--modio-radius);
  border: 1px solid var(--modio-border);
  background: rgba(26, 26, 27, 0.98);
  box-shadow: var(--modio-shadow);
  backdrop-filter: blur(10px);
}

.profile-menu-item {
  display: flex;
  align-items: center;
  gap: 0.65rem;
  width: 100%;
  padding: 0.55rem 0.6rem;
  border: none;
  border-radius: calc(var(--modio-radius-sm) + 1px);
  background: transparent;
  color: var(--modio-text);
  text-align: left;
  box-shadow: none;
}

.profile-menu-item:hover:not(:disabled) {
  background: var(--modio-surface-hover);
  color: var(--modio-text);
}

.profile-menu-item--active {
  background: rgba(var(--modio-accent-rgb), 0.1);
}

.profile-menu-item--active:hover:not(:disabled) {
  background: rgba(var(--modio-accent-rgb), 0.14);
}

.profile-menu-item--disabled {
  opacity: 0.55;
}

.profile-menu-copy {
  display: flex;
  flex-direction: column;
  gap: 0.1rem;
  min-width: 0;
  flex: 1;
}

.profile-menu-check {
  flex-shrink: 0;
  width: 0.45rem;
  height: 0.75rem;
  border-right: 2px solid var(--modio-accent);
  border-bottom: 2px solid var(--modio-accent);
  transform: rotate(45deg);
}

.profile-menu-footer {
  margin-top: 0.25rem;
  padding-top: 0.35rem;
  border-top: 1px solid var(--modio-border);
}

.profile-menu-manage {
  display: block;
  padding: 0.45rem 0.6rem;
  border-radius: var(--modio-radius-sm);
  font-size: 0.78rem;
  font-weight: 500;
  color: var(--modio-text-muted);
  transition:
    background-color 0.2s ease,
    color 0.2s ease;
}

.profile-menu-manage:hover {
  background: var(--modio-surface-hover);
  color: var(--modio-accent);
}

.profile-switcher-status {
  display: flex;
  align-items: center;
  gap: 0.4rem;
  margin: 0.5rem 0.4rem 0;
  font-size: 0.72rem;
  line-height: 1.35;
}

.profile-switcher-status--hint {
  color: var(--modio-text-muted);
}

.profile-switcher-status--error {
  color: var(--modio-danger);
}

.profile-switcher-status--loading {
  color: var(--modio-text-muted);
}

.profile-status-spinner {
  width: 0.75rem;
  height: 0.75rem;
  border: 1.5px solid rgba(156, 163, 175, 0.35);
  border-top-color: var(--modio-accent);
  border-radius: 50%;
  animation: profile-spin 0.7s linear infinite;
}

.profile-menu-enter-active,
.profile-menu-leave-active {
  transition:
    opacity 0.15s ease,
    transform 0.15s ease;
}

.profile-menu-enter-from,
.profile-menu-leave-to {
  opacity: 0;
  transform: translateY(-4px);
}

@keyframes profile-spin {
  to {
    transform: rotate(360deg);
  }
}

.app-sidebar-badge {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  min-width: 1.25rem;
  height: 1.25rem;
  padding: 0 0.35rem;
  border-radius: 999px;
  background: var(--modio-accent);
  color: var(--modio-on-accent);
  font-size: 0.7rem;
  font-weight: 700;
  line-height: 1;
}

.sidebar-play {
  margin: 0 0.35rem 0.85rem;
}

.sidebar-play-btn {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 0.5rem;
  width: 100%;
  padding: 0.65rem 0.75rem;
  border-radius: var(--modio-radius);
  border: 1px solid rgba(var(--modio-accent-rgb), 0.45);
  background:
    linear-gradient(180deg, rgba(var(--modio-accent-rgb), 0.18), rgba(var(--modio-accent-rgb), 0.08)),
    var(--modio-surface-raised);
  color: var(--modio-text);
  font-size: 0.88rem;
  font-weight: 600;
  box-shadow:
    0 1px 2px rgba(0, 0, 0, 0.2),
    inset 0 1px 0 rgba(255, 255, 255, 0.05);
  transition:
    border-color 0.2s ease,
    background 0.2s ease,
    box-shadow 0.2s ease;
}

.sidebar-play-btn:hover:not(:disabled) {
  border-color: rgba(var(--modio-accent-rgb), 0.7);
  background:
    linear-gradient(180deg, rgba(var(--modio-accent-rgb), 0.24), rgba(var(--modio-accent-rgb), 0.1)),
    var(--modio-surface-hover);
  box-shadow:
    0 2px 8px rgba(var(--modio-accent-rgb), 0.12),
    inset 0 1px 0 rgba(255, 255, 255, 0.06);
}

.sidebar-play-btn:disabled {
  opacity: 0.65;
  cursor: not-allowed;
}

.sidebar-play-icon {
  width: 0;
  height: 0;
  border-style: solid;
  border-width: 0.35rem 0 0.35rem 0.55rem;
  border-color: transparent transparent transparent var(--modio-accent);
  margin-left: 0.1rem;
}

.sidebar-play-error {
  margin: 0.45rem 0.4rem 0;
  font-size: 0.72rem;
  line-height: 1.35;
  color: var(--modio-danger);
}
</style>
