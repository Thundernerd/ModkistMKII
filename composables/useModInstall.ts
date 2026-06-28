import { invoke } from "~/utils/tauri";
import { logger } from "~/utils/logger";
import { useNotifications } from "~/composables/useNotifications";

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

export interface InstalledModRecord {
  modId: number;
  fileId: number;
  kind: "plugin" | "blueprint";
  folderName: string;
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
  dependencyFailureCount?: number;
  failedDependencies?: number[];
}

interface InstallModOptions {
  suppressSuccessToast?: boolean;
  versionLabel?: string;
}

const SUCCESS_TOAST_DURATION_MS = 6_000;
const WARNING_TOAST_DURATION_MS = 8_000;
const ERROR_TOAST_DURATION_MS = 8_000;

function isRateLimitedMessage(message: string) {
  return message.toLowerCase().includes("rate limit");
}

function isReadableToastDetail(message: string) {
  const trimmed = message.trim();
  if (!trimmed || trimmed.length > 80) {
    return false;
  }
  if (/error_ref\s*\d+/i.test(trimmed)) {
    return false;
  }
  if (trimmed.toLowerCase().includes("oauth")) {
    return false;
  }
  return true;
}

function subscriptionSyncFailureToast(message: string): {
  title: string;
  message: string;
} {
  const lower = message.toLowerCase();

  if (isRateLimitedMessage(message)) {
    return {
      title: "mod.io rate limit",
      message:
        "Couldn't sync your subscribed mods. Try again in about a minute.",
    };
  }

  if (
    lower.includes("sign in") ||
    lower.includes("not logged in") ||
    lower.includes("authentication")
  ) {
    return {
      title: "Sign in required",
      message: "Sign in to mod.io to sync your subscribed mods.",
    };
  }

  if (lower.includes("zeepkist is running")) {
    return {
      title: "Game is running",
      message: "Close Zeepkist, then return to Mods to sync subscribed mods.",
    };
  }

  if (
    lower.includes("vanilla profile") ||
    lower.includes("installing mods is disabled")
  ) {
    return {
      title: "Wrong profile",
      message: "Switch off the Vanilla profile to install subscribed mods.",
    };
  }

  const trimmed = message.trim();
  if (isReadableToastDetail(trimmed)) {
    return {
      title: "Couldn't sync subscribed mods",
      message: `Couldn't sync your subscribed mods. ${trimmed}`,
    };
  }

  return {
    title: "Couldn't sync subscribed mods",
    message: "Your mod list may be incomplete. Try again in a moment.",
  };
}

function installedModName(modId: number) {
  return installedMods.value.find((mod) => mod.modId === modId)?.name ?? `Mod ${modId}`;
}

function formatCountLabel(count: number, singular: string, plural?: string) {
  const label = count === 1 ? singular : (plural ?? `${singular}s`);
  return `${count} ${label}`;
}

function notifySubscriptionSyncComplete(
  pushNotification: ReturnType<typeof useNotifications>["pushNotification"],
  result: InstallModResult,
  updateCount: number,
) {
  const dependencyFailures = result.dependencyFailureCount ?? 0;
  const installedCount = result.installed.length;

  if (dependencyFailures > 0) {
    if (installedCount > 0) {
      pushNotification({
        title: "Subscriptions synced with warnings",
        message: `Installed ${formatCountLabel(installedCount, "subscribed mod", "subscribed mods")}, but some dependencies could not be installed.`,
        tone: "warning",
        durationMs: WARNING_TOAST_DURATION_MS,
      });
      return;
    }

    if (updateCount === 0) {
      pushNotification({
        title: "Subscriptions synced with warnings",
        message:
          "All subscribed mods are already up to date, but some dependencies could not be installed.",
        tone: "warning",
        durationMs: WARNING_TOAST_DURATION_MS,
      });
      return;
    }

    pushNotification({
      title: "Subscriptions synced with warnings",
      message:
        "Some dependencies could not be installed. See Settings → Sync failures.",
      tone: "warning",
      durationMs: WARNING_TOAST_DURATION_MS,
    });
    return;
  }

  if (installedCount > 0) {
    pushNotification({
      title: "Subscriptions synced",
      message: `Installed ${formatCountLabel(installedCount, "subscribed mod", "subscribed mods")}.`,
      tone: "success",
      durationMs: SUCCESS_TOAST_DURATION_MS,
    });
    return;
  }

  if (updateCount === 0) {
    pushNotification({
      title: "Subscriptions synced",
      message: "All subscribed mods are already up to date.",
      tone: "success",
      durationMs: SUCCESS_TOAST_DURATION_MS,
    });
  }
}

function dependencyCount(result: InstallModResult, modId: number) {
  return result.installed.filter((id) => id !== modId).length;
}

function notifyInstallSuccess(
  pushNotification: ReturnType<typeof useNotifications>["pushNotification"],
  modId: number,
  result: InstallModResult,
  wasUpdate: boolean,
  customFileId?: number,
  versionLabel?: string,
) {
  const depCount = dependencyCount(result, modId);
  const failedDeps = result.failedDependencies ?? [];
  const customVersion = customFileId !== undefined;
  const name = installedModName(modId);
  const versionSuffix = versionLabel ? ` (${versionLabel})` : "";
  const verb = wasUpdate ? "Updated" : "Installed";

  if (failedDeps.length > 0) {
    pushNotification({
      title: wasUpdate ? "Mod updated with warnings" : "Mod installed with warnings",
      message: `${verb} ${name}${versionSuffix}, but ${formatCountLabel(failedDeps.length, "dependency", "dependencies")} could not be installed.`,
      tone: "warning",
      durationMs: WARNING_TOAST_DURATION_MS,
    });
    return;
  }

  if (!wasUpdate && depCount === 0 && !customVersion) {
    return;
  }

  const depSuffix =
    depCount > 0
      ? ` and ${formatCountLabel(depCount, "dependency", "dependencies")}`
      : "";

  pushNotification({
    title: wasUpdate ? "Mod updated" : "Mod installed",
    message: `${verb} ${name}${versionSuffix}${depSuffix}.`,
    tone: "success",
    durationMs: SUCCESS_TOAST_DURATION_MS,
  });
}

function notifyBulkUpdateResult(
  pushNotification: ReturnType<typeof useNotifications>["pushNotification"],
  updated: number[],
  failed: number[],
) {
  if (updated.length > 0 && failed.length === 0) {
    pushNotification({
      title: "Mods updated",
      message: `Updated ${formatCountLabel(updated.length, "mod", "mods")}.`,
      tone: "success",
      durationMs: SUCCESS_TOAST_DURATION_MS,
    });
    return;
  }

  if (updated.length > 0 && failed.length > 0) {
    pushNotification({
      title: "Mods partially updated",
      message: `Updated ${updated.length}, ${failed.length} failed.`,
      tone: "success",
      durationMs: SUCCESS_TOAST_DURATION_MS,
    });
    return;
  }

  if (failed.length > 0) {
    pushNotification({
      title: "Could not update mods",
      message: "Check the errors below and try again.",
      tone: "error",
      durationMs: 10_000,
    });
  }
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
let subscriptionSyncInFlight: { promise: Promise<void> | null } = { promise: null };

const SUBSCRIPTION_SYNC_CANCELLED = "Subscription sync cancelled";

export async function cancelSubscriptionSync() {
  logger.debug("Cancelling subscription sync");
  subscriptionSyncGeneration += 1;
  syncingSubscriptions.value = false;
  await invoke("cancel_subscription_sync").catch(() => {});
}

function resetSessionSync() {
  sessionSyncDone.value = false;
}

function resetStartupUpdateCheck() {
  startupUpdateCheckDone.value = false;
}

async function listInstalledMods(): Promise<InstalledModEntry[]> {
  return invoke<InstalledModEntry[]>("list_installed_mods");
}

/// Mark mods present on disk as installed immediately, without waiting for the
/// network-bound metadata/update lookup. Existing (already enriched) states are
/// left untouched; update availability and uninstall blockers fill in later.
async function seedInstalledFromDisk() {
  try {
    const records = await invoke<InstalledModRecord[]>("list_installed_mod_records");
    const next = { ...installStates.value };
    for (const record of records) {
      if (next[record.modId]) continue;
      next[record.modId] = {
        status: "upToDate",
        installedFileId: record.fileId,
        latestFileId: record.fileId,
        kind: record.kind,
        canUninstall: false,
        uninstallBlockedBy: [],
      };
    }
    installStates.value = next;
    installReady.value = true;
  } catch (error) {
    logger.debug("Could not seed installed mods from disk", error);
  }
}

export function useModInstall() {
  const { pushNotification } = useNotifications();
  const {
    installBlocked: profileInstallBlocked,
    refreshProfiles,
    switching: profileSwitching,
  } = useProfiles();
  const { gameRunning, gameRunningMessage } = useGameProcess();

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
    if (sessionSyncDone.value) {
      return;
    }
    if (subscriptionSyncInFlight.promise) {
      await subscriptionSyncInFlight.promise;
      return;
    }

    subscriptionSyncInFlight.promise = (async () => {
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
        notifySubscriptionSyncComplete(pushNotification, result, updateCount.value);
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
        pushNotification({
          ...subscriptionSyncFailureToast(message),
          tone: "error",
          durationMs: ERROR_TOAST_DURATION_MS,
        });
        await refreshInstalled().catch(() => {});
      } finally {
        if (generation === subscriptionSyncGeneration) {
          syncingSubscriptions.value = false;
        }
      }
    })();

    try {
      await subscriptionSyncInFlight.promise;
    } finally {
      subscriptionSyncInFlight.promise = null;
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

  async function installMod(
    modId: number,
    fileId?: number,
    options?: InstallModOptions,
  ) {
    clearInstallError(modId);
    const wasUpdate = installStates.value[modId]?.status === "updateAvailable";
    setInstalling(modId, true);
    logger.info(`Installing mod ${modId}`, fileId ? { fileId } : undefined);
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

      if (gameRunning.value) {
        throw new Error(
          gameRunningMessage.value ??
            "Zeepkist is running. Close the game before installing, updating, or removing mods.",
        );
      }

      const result = await invoke<InstallModResult>("install_mod", {
        modId,
        fileId: fileId ?? null,
      });
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
      if (!options?.suppressSuccessToast) {
        notifyInstallSuccess(
          pushNotification,
          modId,
          result,
          wasUpdate,
          fileId,
          options?.versionLabel,
        );
      }
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
      if (gameRunning.value) {
        throw new Error(
          gameRunningMessage.value ??
            "Zeepkist is running. Close the game before installing, updating, or removing mods.",
        );
      }
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
    if (gameRunning.value) {
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
    if (gameRunning.value) {
      return false;
    }
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

    await seedInstalledFromDisk();

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
    if (profileInstallBlocked.value || gameRunning.value) {
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
          await installMod(mod.modId, undefined, { suppressSuccessToast: true });
          updated.push(mod.modId);
        } catch {
          failed.push(mod.modId);
        }
      }
      notifyBulkUpdateResult(pushNotification, updated, failed);
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
    bulkUpdating,
    startupUpdateCheckDone,
    profileInstallBlocked,
    gameRunning,
    gameRunningMessage,
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
