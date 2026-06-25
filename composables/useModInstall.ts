import { invoke } from "~/utils/tauri";
import { logger } from "~/utils/logger";

export type InstallUiStatus =
  | "notInstalled"
  | "upToDate"
  | "updateAvailable"
  | "installing"
  | "unavailable"
  | "installBlocked";

export interface UninstallBlocker {
  modId: number;
  name: string;
}

export interface ModInstallState {
  status: "notInstalled" | "upToDate" | "updateAvailable";
  installedFileId: number | null;
  latestFileId: number | null;
  kind: "plugin" | "blueprint" | null;
  canUninstall: boolean;
  uninstallBlockedBy: UninstallBlocker[];
}

export interface InstalledModEntry {
  modId: number;
  fileId: number;
  kind: "plugin" | "blueprint";
  folderName: string;
  name: string;
  summary: string;
  logoUrl: string;
  tags: string[];
  updateAvailable: boolean;
  latestFileId: number | null;
  canUninstall: boolean;
  uninstallBlockedBy: UninstallBlocker[];
}

export interface InstallModResult {
  installed: number[];
  skipped: number[];
}

interface ActiveProfileInfo {
  id: string;
  name: string;
  kind: "vanilla" | "user" | "custom";
  installBlocked: boolean;
}

const installStates = ref<Record<number, ModInstallState>>({});
const installedMods = ref<InstalledModEntry[]>([]);
const installingIds = ref<Set<number>>(new Set());
const uninstallingIds = ref<Set<number>>(new Set());
const installErrors = ref<Record<number, string>>({});
const installReady = ref(false);
const installEnvironmentError = ref("");
const checkingUpdates = ref(false);
const syncingSubscriptions = ref(false);
const syncSubscriptionError = ref("");
const bulkUpdating = ref(false);
const startupUpdateCheckDone = ref(false);

let refreshInFlight: Promise<void> | null = null;

const modsWithUpdates = computed(() =>
  installedMods.value.filter((mod) => mod.updateAvailable),
);

const updateCount = computed(() => modsWithUpdates.value.length);

function setUninstalling(modId: number, uninstalling: boolean) {
  const next = new Set(uninstallingIds.value);
  if (uninstalling) {
    next.add(modId);
  } else {
    next.delete(modId);
  }
  uninstallingIds.value = next;
}

function setInstalling(modId: number, installing: boolean) {
  const next = new Set(installingIds.value);
  if (installing) {
    next.add(modId);
  } else {
    next.delete(modId);
  }
  installingIds.value = next;
}

function clearInstallError(modId: number) {
  if (!installErrors.value[modId]) return;
  const { [modId]: _removed, ...rest } = installErrors.value;
  installErrors.value = rest;
}

function mapInstallState(state: ModInstallState): ModInstallState {
  return {
    status: state.status,
    installedFileId: state.installedFileId,
    latestFileId: state.latestFileId,
    kind: state.kind,
    canUninstall: state.canUninstall,
    uninstallBlockedBy: state.uninstallBlockedBy ?? [],
  };
}

function applyInstalledList(entries: InstalledModEntry[]) {
  installedMods.value = entries;
  const nextStates: Record<number, ModInstallState> = {};
  for (const mod of entries) {
    nextStates[mod.modId] = {
      status: mod.updateAvailable ? "updateAvailable" : "upToDate",
      installedFileId: mod.fileId,
      latestFileId: mod.latestFileId ?? mod.fileId,
      kind: mod.kind,
      canUninstall: mod.canUninstall,
      uninstallBlockedBy: mod.uninstallBlockedBy ?? [],
    };
  }
  installStates.value = nextStates;
  installReady.value = true;
}

const sessionSyncDone = ref(false);

let subscriptionSyncGeneration = 0;

const SUBSCRIPTION_SYNC_CANCELLED = "Subscription sync cancelled";

export async function cancelSubscriptionSync() {
  logger.debug("Cancelling subscription sync");
  subscriptionSyncGeneration += 1;
  syncingSubscriptions.value = false;
  syncSubscriptionError.value = "";
  await invoke("cancel_subscription_sync").catch(() => {});
}

function resetSessionSync() {
  sessionSyncDone.value = false;
  syncSubscriptionError.value = "";
}

function resetStartupUpdateCheck() {
  startupUpdateCheckDone.value = false;
}

async function listInstalledMods(): Promise<InstalledModEntry[]> {
  return invoke<InstalledModEntry[]>("list_installed_mods");
}

export function useModInstall() {
  const {
    installBlocked: profileInstallBlocked,
    refreshProfiles,
    switching: profileSwitching,
  } = useProfiles();

  async function refreshInstalled() {
    if (refreshInFlight) {
      await refreshInFlight;
      return;
    }

    refreshInFlight = (async () => {
      try {
        installEnvironmentError.value = "";
        await refreshProfiles().catch(() => {});

        const authStatus = await invoke<{ loggedIn: boolean }>("auth_status");
        if (!authStatus.loggedIn) {
          resetSessionSync();
        }

        applyInstalledList(await listInstalledMods());
        logger.debug(`Refreshed installed mods (${installedMods.value.length} total)`);
      } catch (error) {
        installReady.value = false;
        installEnvironmentError.value =
          error instanceof Error ? error.message : String(error);
        installedMods.value = [];
        installStates.value = {};
      }
    })();

    try {
      await refreshInFlight;
    } finally {
      refreshInFlight = null;
    }
  }

  async function syncSubscribedModsIfNeeded() {
    if (sessionSyncDone.value || syncingSubscriptions.value) {
      return;
    }

    const authStatus = await invoke<{ loggedIn: boolean }>("auth_status");
    if (!authStatus.loggedIn) {
      return;
    }

    const activeProfile = await invoke<ActiveProfileInfo>("get_active_profile").catch(
      () => null,
    );
    if (activeProfile?.kind !== "user") {
      return;
    }

    const generation = subscriptionSyncGeneration;
    syncingSubscriptions.value = true;
    syncSubscriptionError.value = "";
    logger.info("Starting subscription sync");

    try {
      const result = await invoke<InstallModResult>("sync_subscribed_mods");
      if (generation !== subscriptionSyncGeneration) {
        logger.debug("Subscription sync result ignored (cancelled)");
        return;
      }
      sessionSyncDone.value = true;
      logger.info("Subscription sync complete", result);
      await refreshInstalled();
    } catch (error) {
      if (generation !== subscriptionSyncGeneration) {
        return;
      }
      const message = error instanceof Error ? error.message : String(error);
      if (message === SUBSCRIPTION_SYNC_CANCELLED) {
        logger.debug("Subscription sync cancelled");
        return;
      }
      logger.error("Subscription sync failed", message);
      syncSubscriptionError.value = message;
      await refreshInstalled().catch(() => {});
    } finally {
      if (generation === subscriptionSyncGeneration) {
        syncingSubscriptions.value = false;
      }
    }
  }

  async function refreshInstallState(modId: number) {
    try {
      const state = mapInstallState(
        await invoke<ModInstallState>("get_mod_install_state", {
          modId,
        }),
      );
      installStates.value = { ...installStates.value, [modId]: state };
      installEnvironmentError.value = "";
      installReady.value = true;
      return state;
    } catch (error) {
      installEnvironmentError.value =
        error instanceof Error ? error.message : String(error);
      return null;
    }
  }

  async function installMod(modId: number) {
    clearInstallError(modId);
    setInstalling(modId, true);
    logger.info(`Installing mod ${modId}`);
    try {
      if (profileSwitching.value) {
        throw new Error("Wait for the profile switch to finish, then try again.");
      }

      await cancelSubscriptionSync();

      const activeProfile = await invoke<ActiveProfileInfo>("get_active_profile");
      if (activeProfile.installBlocked) {
        throw new Error(
          "Installing mods is disabled on the Vanilla profile. Switch to your account profile in the sidebar.",
        );
      }

      const result = await invoke<InstallModResult>("install_mod", { modId });
      if (activeProfile.kind === "user") {
        sessionSyncDone.value = true;
      }

      applyInstalledList(await listInstalledMods());
      installEnvironmentError.value = "";

      const refreshIds = new Set([modId, ...result.installed, ...result.skipped]);
      for (const id of refreshIds) {
        await refreshInstallState(id);
      }
      logger.info(`Install finished for mod ${modId}`, result);
      return result;
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      logger.error(`Install failed for mod ${modId}`, message);
      installErrors.value = { ...installErrors.value, [modId]: message };
      throw error;
    } finally {
      setInstalling(modId, false);
    }
  }

  async function uninstallMod(modId: number) {
    clearInstallError(modId);
    setUninstalling(modId, true);
    logger.info(`Uninstalling mod ${modId}`);
    try {
      await cancelSubscriptionSync();
      await invoke("uninstall_mod", { modId });
      await refreshInstalled();
      logger.info(`Uninstalled mod ${modId}`);
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      logger.error(`Uninstall failed for mod ${modId}`, message);
      installErrors.value = { ...installErrors.value, [modId]: message };
      throw error;
    } finally {
      setUninstalling(modId, false);
    }
  }

  function getUiStatus(modId: number): InstallUiStatus {
    if (!installReady.value && installEnvironmentError.value) {
      return "unavailable";
    }
    if (uninstallingIds.value.has(modId) || installingIds.value.has(modId)) {
      return "installing";
    }
    const state = installStates.value[modId];
    if (profileSwitching.value) {
      if (state?.status === "upToDate") {
        return "upToDate";
      }
      return "installBlocked";
    }
    if (profileInstallBlocked.value) {
      if (!state || state.status !== "upToDate") {
        return "installBlocked";
      }
    }
    if (!state) return "notInstalled";
    return state.status;
  }

  function getCanUninstall(modId: number) {
    return installStates.value[modId]?.canUninstall ?? false;
  }

  function getInstallError(modId: number) {
    return installErrors.value[modId] ?? "";
  }

  function isUninstalling(modId: number) {
    return uninstallingIds.value.has(modId);
  }

  async function checkForUpdatesOnStartup() {
    if (startupUpdateCheckDone.value) {
      return installedMods.value;
    }

    checkingUpdates.value = true;
    try {
      await refreshInstalled();
      startupUpdateCheckDone.value = true;
      return installedMods.value;
    } finally {
      checkingUpdates.value = false;
    }
  }

  async function updateAllMods() {
    if (profileInstallBlocked.value) {
      return { updated: [] as number[], failed: [] as number[] };
    }

    const targets = [...modsWithUpdates.value];
    if (targets.length === 0) {
      return { updated: [] as number[], failed: [] as number[] };
    }

    bulkUpdating.value = true;
    const updated: number[] = [];
    const failed: number[] = [];

    try {
      for (const mod of targets) {
        try {
          await installMod(mod.modId);
          updated.push(mod.modId);
        } catch {
          failed.push(mod.modId);
        }
      }
    } finally {
      bulkUpdating.value = false;
    }

    return { updated, failed };
  }

  return {
    installedMods,
    modsWithUpdates,
    updateCount,
    installStates,
    installingIds,
    uninstallingIds,
    installErrors,
    installReady,
    installEnvironmentError,
    checkingUpdates,
    syncingSubscriptions,
    syncSubscriptionError,
    bulkUpdating,
    startupUpdateCheckDone,
    profileInstallBlocked,
    refreshInstalled,
    resetSessionSync,
    resetStartupUpdateCheck,
    cancelSubscriptionSync,
    syncSubscribedModsIfNeeded,
    checkForUpdatesOnStartup,
    refreshInstallState,
    installMod,
    uninstallMod,
    updateAllMods,
    getUiStatus,
    getCanUninstall,
    getInstallError,
    isUninstalling,
  };
}
