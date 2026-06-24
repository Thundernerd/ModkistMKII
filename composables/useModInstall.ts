import { invoke } from "~/utils/tauri";

export type InstallUiStatus =
  | "notInstalled"
  | "upToDate"
  | "updateAvailable"
  | "installing"
  | "unavailable";

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

const installStates = ref<Record<number, ModInstallState>>({});
const installedMods = ref<InstalledModEntry[]>([]);
const installingIds = ref<Set<number>>(new Set());
const uninstallingIds = ref<Set<number>>(new Set());
const installErrors = ref<Record<number, string>>({});
const installReady = ref(false);
const installEnvironmentError = ref("");

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

export function useModInstall() {
  async function refreshInstalled() {
    try {
      installEnvironmentError.value = "";
      installedMods.value = await invoke<InstalledModEntry[]>("list_installed_mods");
      const nextStates: Record<number, ModInstallState> = {};
      for (const mod of installedMods.value) {
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
    } catch (error) {
      installReady.value = false;
      installEnvironmentError.value =
        error instanceof Error ? error.message : String(error);
      installedMods.value = [];
      installStates.value = {};
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
    try {
      const result = await invoke<InstallModResult>("install_mod", { modId });
      await refreshInstalled();
      for (const id of [...result.installed, ...result.skipped]) {
        await refreshInstallState(id);
      }
      return result;
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      installErrors.value = { ...installErrors.value, [modId]: message };
      throw error;
    } finally {
      setInstalling(modId, false);
    }
  }

  async function uninstallMod(modId: number) {
    clearInstallError(modId);
    setUninstalling(modId, true);
    try {
      await invoke("uninstall_mod", { modId });
      await refreshInstalled();
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
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

  return {
    installedMods,
    installStates,
    installingIds,
    uninstallingIds,
    installErrors,
    installReady,
    installEnvironmentError,
    refreshInstalled,
    refreshInstallState,
    installMod,
    uninstallMod,
    getUiStatus,
    getCanUninstall,
    getInstallError,
    isUninstalling,
  };
}
