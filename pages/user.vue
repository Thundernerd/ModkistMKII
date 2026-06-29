<script setup lang="ts">
definePageMeta({ layout: "app" });

const {
  authStatus,
  refreshAuthStatus,
  checkLogoutRequiresProfileSelection,
  completeLogout,
} = useModioAuth();
const { profile, userMods, loading, error, fetchUserData } = useUserProfile();
const {
  switchProfile,
  logoutPickerProfiles,
  refreshProfiles,
} = useProfiles();
const { invalidateInstalledModsCache } = useModInstall();

const profilePickerOpen = ref(false);
const logoutError = ref("");

async function handleLogout() {
  logoutError.value = "";
  try {
    const needsPicker = await checkLogoutRequiresProfileSelection();
    if (needsPicker) {
      await refreshProfiles();
      profilePickerOpen.value = true;
      return;
    }

    await completeLogout();
    await navigateTo("/");
  } catch (err) {
    logoutError.value = err instanceof Error ? err.message : String(err);
  }
}

async function handleLogoutProfileSelect(profileId: string) {
  logoutError.value = "";
  try {
    await switchProfile(profileId);
    profilePickerOpen.value = false;
    await completeLogout();
    invalidateInstalledModsCache();
    await navigateTo("/");
  } catch (err) {
    logoutError.value = err instanceof Error ? err.message : String(err);
  }
}

onMounted(async () => {
  await refreshAuthStatus();
  if (!authStatus.value.loggedIn) {
    await navigateTo("/?redirect=/user");
    return;
  }
  await fetchUserData();
});
</script>

<template>
  <div class="page">
    <header class="page-header">
      <h1>User</h1>
    </header>

    <div v-if="loading && !profile" class="state">
      <span class="spinner" aria-hidden="true" />
      Loading profile…
    </div>

    <template v-else-if="authStatus.loggedIn">
      <section class="profile-card panel">
        <div v-if="profile" class="profile-header">
          <img
            v-if="profile.avatarUrl"
            :src="profile.avatarUrl"
            :alt="`${profile.username} avatar`"
            class="profile-avatar"
          />
          <div v-else class="profile-avatar profile-avatar--fallback" />
          <div>
            <h2 class="profile-name">{{ profile.username }}</h2>
            <a
              :href="profile.profileUrl"
              target="_blank"
              rel="noopener noreferrer"
              class="profile-link"
            >
              View on mod.io
            </a>
          </div>
        </div>
        <div v-else class="profile-header">
          <div class="profile-avatar profile-avatar--fallback" />
          <div>
            <h2 class="profile-name">{{ authStatus.username ?? "Account" }}</h2>
            <p v-if="error" class="hint profile-warning">
              Profile details could not be loaded right now.
            </p>
          </div>
        </div>
        <button type="button" class="btn-secondary logout-button" @click="handleLogout">
          Log out
        </button>
      </section>

      <p v-if="error" class="error">{{ error }}</p>
      <p v-if="logoutError" class="error">{{ logoutError }}</p>

      <section v-if="profile" class="user-mods">
        <h2 class="section-title">Your mods</h2>
        <p v-if="userMods.total" class="meta mods-count">
          {{ userMods.total }} mod{{ userMods.total === 1 ? "" : "s" }} for this game
        </p>

        <div v-if="loading" class="state state--compact">
          <span class="spinner" aria-hidden="true" />
          Loading mods…
        </div>

        <p v-else-if="userMods.mods.length === 0" class="hint empty-state">
          You haven't submitted any mods for this game.
        </p>

        <ul v-else class="mod-grid">
          <li v-for="mod in userMods.mods" :key="mod.id">
            <ModCard :mod="mod" />
          </li>
        </ul>
      </section>
    </template>

    <ProfilePickerDialog
      :open="profilePickerOpen"
      :profiles="logoutPickerProfiles()"
      title="Switch profile before logging out"
      description="Choose which profile should be active after you sign out."
      @close="profilePickerOpen = false"
      @select="handleLogoutProfileSelect"
    />
  </div>
</template>

<style scoped>
.page-header {
  margin-bottom: 1.5rem;
}

.page-header h1 {
  margin: 0;
  font-size: 1.5rem;
  font-weight: 700;
  letter-spacing: -0.02em;
}

.panel {
  padding: 1.5rem;
  border-radius: var(--modio-radius);
  background: var(--modio-surface);
  border: 1px solid var(--modio-border);
  box-shadow: var(--modio-shadow);
}

.profile-card {
  margin-bottom: 1.5rem;
}

.profile-header {
  display: flex;
  align-items: center;
  gap: 1rem;
  margin-bottom: 1rem;
}

.profile-avatar {
  width: 4rem;
  height: 4rem;
  border-radius: 50%;
  object-fit: cover;
  background: var(--modio-surface-raised);
}

.profile-avatar--fallback {
  border: 1px solid var(--modio-border);
}

.profile-name {
  margin: 0 0 0.25rem;
  font-size: 1.15rem;
  font-weight: 600;
}

.profile-link {
  font-size: 0.9rem;
  color: var(--modio-accent);
}

.profile-warning {
  margin: 0.35rem 0 0;
}

.logout-button {
  margin-top: 0.25rem;
}

.section-title {
  margin: 0 0 0.5rem;
  font-size: 1.1rem;
  font-weight: 600;
}

.mods-count {
  margin: 0 0 1rem;
}

.mod-grid {
  list-style: none;
  margin: 0;
  padding: 0;
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(15.5rem, 1fr));
  gap: 1rem;
}

.state {
  display: flex;
  align-items: center;
  gap: 0.65rem;
  color: var(--modio-text-muted);
}

.state--compact {
  padding: 0.5rem 0;
}

.empty-state {
  margin: 0;
}
</style>
