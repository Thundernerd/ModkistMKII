<script setup lang="ts">
import { confirm } from "@tauri-apps/plugin-dialog";
import { revealItemInDir } from "@tauri-apps/plugin-opener";
import { wineWinhttpFeedback } from "~/utils/wineWinhttp";
import { invoke } from "~/utils/tauri";
import { BEPINEX_REQUIRED_VERSION, useBepInEx } from "~/composables/useBepInEx";

definePageMeta({ layout: "app" });

const {
  bepinexStatus,
  loading,
  installing,
  error,
  refreshBepInExStatus,
  verifyBepInEx,
  reinstallBepInEx,
} = useBepInEx();
const {
  autoUpdateMods,
  ignoreBepInExVersionWarning,
  settingsReady,
  refreshAppSettings,
  setAutoUpdateMods,
} = useAppSettings();

const verifyMessage = ref("");
const verifyTone = ref<"info" | "success" | "error" | "warn">("info");

const wineFeedback = computed(() =>
  wineWinhttpFeedback(bepinexStatus.value.wineWinhttp),
);

const statusLabel = computed(() => {
  switch (bepinexStatus.value.state) {
    case "installed":
      return bepinexStatus.value.foundVersion
        ? `Installed (v${bepinexStatus.value.foundVersion})`
        : `Installed (v${BEPINEX_REQUIRED_VERSION})`;
    case "wrongVersion":
      return bepinexStatus.value.foundVersion
        ? `Wrong version (v${bepinexStatus.value.foundVersion})`
        : "Wrong version";
    case "missing":
    default:
      return "Not installed";
  }
});

const statusClass = computed(() => {
  switch (bepinexStatus.value.state) {
    case "installed":
      return "status-ok";
    case "wrongVersion":
      return ignoreBepInExVersionWarning.value ? "status-ok" : "status-warn";
    default:
      return "status-bad";
  }
});

function setVerifyResult(
  tone: "info" | "success" | "error" | "warn",
  message: string,
) {
  verifyTone.value = tone;
  verifyMessage.value = message;
}

async function verifyBepInExInstall() {
  verifyMessage.value = "";
  error.value = "";

  try {
    await verifyBepInEx();
    const status = bepinexStatus.value;

    if (status.state === "installed") {
      const feedback = wineWinhttpFeedback(status.wineWinhttp);
      if (feedback && feedback.tone !== "success") {
        setVerifyResult(feedback.tone === "warn" ? "warn" : "error", feedback.text);
        return;
      }
      setVerifyResult(
        "success",
        status.foundVersion
          ? `BepInEx ${status.foundVersion} is installed correctly.`
          : `BepInEx ${BEPINEX_REQUIRED_VERSION} (x64) is installed correctly.`,
      );
      return;
    }

    if (status.state === "wrongVersion") {
      if (ignoreBepInExVersionWarning.value) {
        setVerifyResult(
          "info",
          status.foundVersion
            ? `BepInEx ${status.foundVersion} is installed. Version warnings are suppressed.`
            : `A different BepInEx version is installed. Version warnings are suppressed.`,
        );
        return;
      }

      setVerifyResult(
        "error",
        status.message ||
          `Expected BepInEx ${BEPINEX_REQUIRED_VERSION} (x64), but a different version was found.`,
      );
      return;
    }

    setVerifyResult(
      "error",
      status.message || "BepInEx is not installed in your game directory.",
    );
  } catch (err) {
    setVerifyResult("error", String(err));
  }
}

async function reinstallBepInExInstall() {
  const confirmed = await confirm(
    `This will remove the entire BepInEx installation from your game folder, including all mods in BepInEx/plugins. Any custom plugins and/or blueprints not managed through Modkist will be lost forever and cannot be recovered. A fresh BepInEx ${BEPINEX_REQUIRED_VERSION} (x64) will then be installed.`,
    { title: "Reinstall BepInEx?", kind: "warning" },
  );

  if (!confirmed) {
    return;
  }

  verifyMessage.value = "";
  error.value = "";

    try {
    await reinstallBepInEx();
    const feedback = wineWinhttpFeedback(bepinexStatus.value.wineWinhttp);
    if (feedback && feedback.tone !== "success") {
      setVerifyResult(feedback.tone === "warn" ? "warn" : "error", feedback.text);
      return;
    }
    setVerifyResult(
      "success",
      bepinexStatus.value.foundVersion
        ? `Reinstalled BepInEx ${bepinexStatus.value.foundVersion}.`
        : `Reinstalled BepInEx ${BEPINEX_REQUIRED_VERSION} (x64).`,
    );
  } catch (err) {
    setVerifyResult("error", String(err));
  }
}

onMounted(() => {
  refreshBepInExStatus();
  refreshProfiles().catch(() => {});
  refreshAppSettings().catch(() => {});
});

const {
  profiles,
  loading: profilesLoading,
  error: profilesError,
  refreshProfiles,
  createProfile,
  deleteProfile,
} = useProfiles();
const { resetStartupUpdateCheck, refreshInstalled } = useModInstall();

const savingAutoUpdate = ref(false);
const autoUpdateError = ref("");
const openingLogsFolder = ref(false);
const logsFolderError = ref("");

async function openLogsFolder() {
  openingLogsFolder.value = true;
  logsFolderError.value = "";

  try {
    const logDir = await invoke<string>("log_directory_path");
    await revealItemInDir(logDir);
  } catch (err) {
    logsFolderError.value = err instanceof Error ? err.message : String(err);
  } finally {
    openingLogsFolder.value = false;
  }
}

async function handleAutoUpdateToggle() {
  const next = !autoUpdateMods.value;
  savingAutoUpdate.value = true;
  autoUpdateError.value = "";
  try {
    await setAutoUpdateMods(next);
  } catch (err) {
    autoUpdateError.value = err instanceof Error ? err.message : String(err);
  } finally {
    savingAutoUpdate.value = false;
  }
}

const newProfileName = ref("");
const profileActionError = ref("");
const creatingProfile = ref(false);

async function handleCreateProfile() {
  const name = newProfileName.value.trim();
  if (!name) return;

  creatingProfile.value = true;
  profileActionError.value = "";
  try {
    await createProfile(name);
    newProfileName.value = "";
    resetStartupUpdateCheck();
    await refreshInstalled();
  } catch (err) {
    profileActionError.value = err instanceof Error ? err.message : String(err);
  } finally {
    creatingProfile.value = false;
  }
}

async function handleDeleteProfile(profileId: string, profileName: string) {
  const confirmed = await confirm(
    `Delete profile "${profileName}"? Its saved mod list will be removed.`,
    { title: "Delete profile?", kind: "warning" },
  );
  if (!confirmed) return;

  profileActionError.value = "";
  try {
    await deleteProfile(profileId);
  } catch (err) {
    profileActionError.value = err instanceof Error ? err.message : String(err);
  }
}

function profileKindLabel(kind: string) {
  if (kind === "vanilla") return "Built-in";
  if (kind === "user") return "Built-in";
  return "Custom";
}
</script>

<template>
  <div class="page">
    <header class="page-header">
      <h1>Settings</h1>
      <p class="hint">Manage your Modkist preferences.</p>
    </header>

    <section class="panel">
      <h2 class="panel-title">Mods</h2>
      <p class="hint panel-desc">
        Control how Modkist keeps your installed mods up to date.
      </p>

      <div class="setting-row">
        <div class="setting-copy">
          <span class="setting-label">Auto-update mods</span>
          <span class="setting-hint">
            When enabled, subscribed mods are updated automatically during sync.
            You can still update mods manually from the Updates page.
          </span>
        </div>
        <button
          type="button"
          class="setting-toggle"
          role="switch"
          :aria-checked="autoUpdateMods"
          :disabled="!settingsReady || savingAutoUpdate"
          @click="handleAutoUpdateToggle"
        >
          <span class="setting-toggle-track" :class="{ on: autoUpdateMods }">
            <span class="setting-toggle-thumb" />
          </span>
        </button>
      </div>

      <p v-if="autoUpdateError" class="error feedback">{{ autoUpdateError }}</p>

      <div class="setting-subsection">
        <div class="setting-row">
          <div class="setting-copy">
            <span class="setting-label">Failed sync mods</span>
            <span class="setting-hint">
              Subscribed mods that could not be installed during sync. You can
              ignore them or unsubscribe on mod.io.
            </span>
          </div>
          <button
            type="button"
            class="btn-secondary setting-action-btn"
            @click="navigateTo('/settings/sync-failures')"
          >
            View list
          </button>
        </div>
      </div>
    </section>

    <section class="panel">
      <h2 class="panel-title">Game directory</h2>
      <p class="hint panel-desc">
        Folder that contains <code>zeepkist.exe</code>.
      </p>
      <GamePathForm input-id="settings-game-path" />
    </section>

    <section class="panel">
      <h2 class="panel-title">Logs</h2>
      <p class="hint panel-desc">
        Modkist writes rotating log files here for troubleshooting.
      </p>
      <button
        type="button"
        class="btn-secondary"
        :disabled="openingLogsFolder"
        @click="openLogsFolder"
      >
        {{ openingLogsFolder ? "Opening…" : "Open logs folder" }}
      </button>
      <p v-if="logsFolderError" class="error feedback">{{ logsFolderError }}</p>
    </section>

    <section class="panel">
      <h2 class="panel-title">Profiles</h2>
      <p class="hint panel-desc">
        Each profile keeps its own installed mod list. Vanilla blocks installs.
      </p>

      <p v-if="profilesError" class="error feedback">{{ profilesError }}</p>
      <p v-if="profileActionError" class="error feedback">{{ profileActionError }}</p>

      <ul v-if="profiles.length" class="profile-list">
        <li
          v-for="profile in profiles"
          :key="profile.id"
          class="profile-list-item"
        >
          <div class="profile-list-main">
            <span class="profile-list-name">{{ profile.name }}</span>
            <span class="profile-list-meta">
              {{ profileKindLabel(profile.kind) }}
              <template v-if="profile.isActive"> · Active</template>
              <template v-if="profile.installBlocked"> · No installs</template>
            </span>
          </div>
          <button
            v-if="profile.kind === 'custom' && !profile.isActive"
            type="button"
            class="btn-danger profile-delete-btn"
            :disabled="profilesLoading"
            @click="handleDeleteProfile(profile.id, profile.name)"
          >
            Delete
          </button>
        </li>
      </ul>

      <form class="profile-create-form" @submit.prevent="handleCreateProfile">
        <label class="profile-create-label" for="new-profile-name">
          New profile
        </label>
        <div class="profile-create-row">
          <input
            id="new-profile-name"
            v-model="newProfileName"
            type="text"
            class="profile-create-input"
            placeholder="Profile name"
            :disabled="creatingProfile"
          />
          <button
            type="submit"
            class="btn-secondary"
            :disabled="creatingProfile || !newProfileName.trim()"
          >
            {{ creatingProfile ? "Adding…" : "Add" }}
          </button>
        </div>
      </form>
    </section>

    <section class="panel">
      <h2 class="panel-title">BepInEx</h2>
      <p class="hint panel-desc">
        Modkist requires BepInEx {{ BEPINEX_REQUIRED_VERSION }} (x64) in your game
        folder.
      </p>

      <p class="status-line">
        Status:
        <span :class="['status-badge', statusClass]">{{ statusLabel }}</span>
      </p>

      <div class="action-row">
        <button
          type="button"
          class="btn-secondary"
          :disabled="loading || installing"
          @click="verifyBepInExInstall"
        >
          {{ loading ? "Verifying…" : "Verify installation" }}
        </button>
        <button
          type="button"
          class="btn-danger"
          :disabled="loading || installing"
          @click="reinstallBepInExInstall"
        >
          {{ installing ? "Reinstalling…" : "Reinstall" }}
        </button>
      </div>

      <p
        v-if="verifyMessage"
        :class="verifyTone === 'success' ? 'info' : verifyTone === 'warn' ? 'warn' : 'error'"
        class="feedback"
      >
        {{ verifyMessage }}
      </p>
      <p
        v-else-if="wineFeedback"
        :class="wineFeedback.tone === 'success' ? 'info' : wineFeedback.tone === 'warn' ? 'warn' : 'error'"
        class="feedback"
      >
        {{ wineFeedback.text }}
      </p>
      <p v-else-if="error" class="error feedback">{{ error }}</p>
    </section>
  </div>
</template>

<style scoped>
.page {
  width: 100%;
}

.page-header {
  margin-bottom: 1.5rem;
}

.page-header h1 {
  margin: 0 0 0.35rem;
  font-size: 1.5rem;
  font-weight: 700;
  letter-spacing: -0.02em;
}

.page-header p {
  margin: 0;
}

.panel {
  padding: 1.5rem;
  border-radius: var(--modio-radius);
  background: var(--modio-surface);
  border: 1px solid var(--modio-border);
  box-shadow: var(--modio-shadow);
}

.panel + .panel {
  margin-top: 1rem;
}

.panel-title {
  margin: 0 0 0.35rem;
  font-size: 1rem;
  font-weight: 600;
}

.panel-desc {
  margin: 0 0 1rem;
}

.panel-desc code {
  color: var(--modio-text);
  font-size: 0.85em;
}

.status-line {
  margin: 0 0 1rem;
  font-size: 0.9rem;
  color: var(--modio-text-muted);
}

.status-badge {
  display: inline-block;
  margin-left: 0.35rem;
  padding: 0.15rem 0.5rem;
  border-radius: 999px;
  font-size: 0.8rem;
  font-weight: 600;
}

.status-ok {
  background: rgba(74, 222, 128, 0.12);
  color: var(--modio-success);
}

.status-warn {
  background: rgba(251, 191, 36, 0.12);
  color: #fbbf24;
}

.status-bad {
  background: rgba(248, 113, 113, 0.12);
  color: var(--modio-danger);
}

.action-row {
  display: flex;
  flex-wrap: wrap;
  gap: 0.75rem;
}

.feedback {
  margin: 0.85rem 0 0;
}

.feedback.warn {
  color: #fbbf24;
}

.profile-list {
  list-style: none;
  margin: 0 0 1rem;
  padding: 0;
  display: flex;
  flex-direction: column;
  gap: 0.65rem;
}

.profile-list-item {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 0.75rem;
  padding: 0.75rem 0.85rem;
  border: 1px solid var(--modio-border);
  border-radius: var(--modio-radius-sm);
}

.profile-list-main {
  display: flex;
  flex-direction: column;
  gap: 0.15rem;
}

.profile-list-name {
  font-weight: 600;
}

.profile-list-meta {
  font-size: 0.8rem;
  color: var(--modio-text-muted);
}

.profile-delete-btn {
  flex-shrink: 0;
  padding: 0.35rem 0.65rem;
  font-size: 0.8rem;
}

.profile-create-form {
  margin-top: 0.5rem;
}

.profile-create-label {
  display: block;
  margin-bottom: 0.35rem;
  font-size: 0.85rem;
  font-weight: 600;
}

.profile-create-row {
  display: flex;
  gap: 0.65rem;
}

.profile-create-input {
  flex: 1;
  padding: 0.5rem 0.65rem;
  border-radius: var(--modio-radius-sm);
  border: 1px solid var(--modio-border);
  background: var(--modio-surface-raised);
  color: var(--modio-text);
}

.setting-row {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 1rem;
}

.setting-copy {
  display: flex;
  flex-direction: column;
  gap: 0.25rem;
  min-width: 0;
  flex: 1;
}

.setting-label {
  font-weight: 600;
}

.setting-hint {
  font-size: 0.85rem;
  color: var(--modio-text-muted);
}

.setting-subsection {
  margin-top: 1rem;
  padding-top: 1rem;
  border-top: 1px solid var(--modio-border);
}

.setting-action-btn {
  flex-shrink: 0;
  align-self: center;
}

.setting-toggle {
  flex-shrink: 0;
  padding: 0;
  border: none;
  background: transparent;
  cursor: pointer;
}

.setting-toggle:hover:not(:disabled) {
  background: transparent;
  border-color: transparent;
}

.setting-toggle:disabled {
  cursor: not-allowed;
  opacity: 0.6;
}

.setting-toggle-track {
  display: block;
  width: 2.75rem;
  height: 1.5rem;
  border-radius: 999px;
  background: var(--modio-surface-raised);
  border: 1px solid var(--modio-border);
  position: relative;
  transition: background-color 0.2s ease, border-color 0.2s ease;
}

.setting-toggle-track.on {
  background: rgba(var(--modio-accent-rgb), 0.22);
  border-color: rgba(var(--modio-accent-rgb), 0.55);
}

.setting-toggle-thumb {
  position: absolute;
  top: 0.125rem;
  left: 0.125rem;
  width: 1.125rem;
  height: 1.125rem;
  border-radius: 50%;
  background: var(--modio-text-muted);
  transition: transform 0.2s ease, background-color 0.2s ease;
}

.setting-toggle-track.on .setting-toggle-thumb {
  transform: translateX(1.25rem);
  background: var(--modio-accent);
}

@media (max-width: 640px) {
  .setting-row {
    flex-direction: column;
    align-items: stretch;
  }

  .setting-action-btn {
    align-self: flex-start;
  }
}
</style>
