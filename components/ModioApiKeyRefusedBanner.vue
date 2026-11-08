<script setup lang="ts">
import { openUrl } from "@tauri-apps/plugin-opener";
import { logger } from "~/utils/logger";

const RELEASES_URL = "https://github.com/Thundernerd/ModkistMKII/releases/latest";

const { modioApiKeyRefused } = useModioStatus();
const { installUpdateNow } = useAppUpdater();

type UpdateCheck = "idle" | "checking" | "none" | "failed";
const updateCheck = ref<UpdateCheck>("idle");

const updateCheckText = computed(() => {
  switch (updateCheck.value) {
    case "checking":
      return "Checking for updates…";
    case "none":
      return "No in-app update is available yet. Download the latest release from GitHub.";
    case "failed":
      return "Could not check for updates. Download the latest release from GitHub.";
    default:
      return "";
  }
});

async function updateNow() {
  updateCheck.value = "checking";
  try {
    const status = await installUpdateNow();
    // "installed" restarts the app; anything else means nothing to install.
    updateCheck.value = status === "installed" ? "idle" : "none";
  } catch (err) {
    logger.debug("Did not install update from the API key banner", err);
    updateCheck.value = "failed";
  }
}

function openReleases() {
  openUrl(RELEASES_URL).catch((err) => {
    logger.debug("Did not open the Modkist releases page", err);
  });
}
</script>

<template>
  <aside
    v-if="modioApiKeyRefused"
    class="api-key-refused-banner"
    role="alert"
  >
    <div class="api-key-refused-banner-body">
      <p class="api-key-refused-banner-title">Update Modkist</p>
      <p class="api-key-refused-banner-text">
        This version of Modkist can no longer reach mod.io. Update Modkist to
        keep browsing and installing mods.
      </p>
      <p v-if="updateCheckText" class="api-key-refused-banner-text">
        {{ updateCheckText }}
      </p>
    </div>
    <div class="api-key-refused-banner-actions">
      <button
        type="button"
        :disabled="updateCheck === 'checking'"
        @click="updateNow"
      >
        Update now
      </button>
      <button
        type="button"
        class="api-key-refused-banner-secondary"
        @click="openReleases"
      >
        Download from GitHub
      </button>
    </div>
  </aside>
</template>

<style scoped>
.api-key-refused-banner {
  display: flex;
  flex-wrap: wrap;
  align-items: flex-start;
  justify-content: space-between;
  gap: 0.85rem 1rem;
  margin: 0 0 1.25rem;
  padding: 0.9rem 1rem;
  border-radius: var(--modio-radius);
  border: 1px solid rgba(248, 113, 113, 0.45);
  background: rgba(248, 113, 113, 0.1);
}

.api-key-refused-banner-body {
  flex: 1 1 16rem;
  min-width: 0;
}

.api-key-refused-banner-title {
  margin: 0 0 0.35rem;
  font-size: 0.95rem;
  font-weight: 650;
  color: #f87171;
}

.api-key-refused-banner-text {
  margin: 0;
  font-size: 0.875rem;
  line-height: 1.45;
  color: var(--modio-text);
}

.api-key-refused-banner-text + .api-key-refused-banner-text {
  margin-top: 0.35rem;
}

.api-key-refused-banner-actions {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 0.65rem;
  flex-shrink: 0;
}

.api-key-refused-banner-secondary {
  background: transparent;
  border: 1px solid var(--modio-border);
  color: var(--modio-text);
}
</style>
