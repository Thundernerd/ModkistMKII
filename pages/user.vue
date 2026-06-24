<script setup lang="ts">
definePageMeta({ layout: "app" });

const { authStatus, refreshAuthStatus, logout } = useModioAuth();
const { profile, userMods, loading, error, fetchUserData } = useUserProfile();

async function handleLogout() {
  await logout();
  await navigateTo("/");
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

    <p v-else-if="error" class="error">{{ error }}</p>

    <template v-else-if="profile">
      <section class="profile-card panel">
        <div class="profile-header">
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
        <button type="button" class="btn-secondary logout-button" @click="handleLogout">
          Log out
        </button>
      </section>

      <section class="user-mods">
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
  </div>
</template>

<style scoped>
.page {
  max-width: 72rem;
}

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
  margin-bottom: 2rem;
}

.profile-header {
  display: flex;
  align-items: center;
  gap: 1rem;
  margin-bottom: 1.25rem;
}

.profile-avatar {
  width: 4rem;
  height: 4rem;
  border-radius: 999px;
  object-fit: cover;
  flex-shrink: 0;
}

.profile-avatar--fallback {
  background: linear-gradient(
    135deg,
    var(--modio-surface-raised),
    var(--modio-surface-hover)
  );
}

.profile-name {
  margin: 0 0 0.25rem;
  font-size: 1.2rem;
  font-weight: 600;
}

.profile-link {
  font-size: 0.9rem;
}

.logout-button {
  width: fit-content;
}

.section-title {
  margin: 0 0 0.35rem;
  font-size: 1.1rem;
  font-weight: 600;
}

.mods-count {
  margin: 0 0 1rem;
}

.empty-state {
  padding: 2rem;
  text-align: center;
  border: 1px dashed var(--modio-border);
  border-radius: var(--modio-radius);
  background: var(--modio-surface);
}

.state {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 0.75rem;
  padding: 4rem 1rem;
  color: var(--modio-text-muted);
}

.state--compact {
  padding: 2rem 1rem;
}

.spinner {
  width: 1.1rem;
  height: 1.1rem;
  border: 2px solid var(--modio-border);
  border-top-color: var(--modio-accent);
  border-radius: 50%;
  animation: spin 0.7s linear infinite;
}

@keyframes spin {
  to {
    transform: rotate(360deg);
  }
}

.mod-grid {
  list-style: none;
  margin: 0;
  padding: 0;
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(15.5rem, 1fr));
  gap: 1rem;
}
</style>
