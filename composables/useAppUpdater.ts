import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { exit, relaunch } from "@tauri-apps/plugin-process";
import { invoke, waitForTauri } from "~/utils/tauri";

export type AppUpdateGatePhase =
  | "checking"
  | "needsPassword"
  | "downloading"
  | "failed"
  | "restarting";

export interface AppUpdateCheckResult {
  status:
    | "skipped"
    | "upToDate"
    | "needsPassword"
    | "installFailed"
    | "installed"
    | string;
  version: string | null;
}

interface AppUpdateAvailablePayload {
  version: string;
}

interface AppUpdateProgressPayload {
  downloaded: number;
  total: number | null;
}

const APP_UPDATE_AVAILABLE_EVENT = "app-update://available";
const APP_UPDATE_PROGRESS_EVENT = "app-update://progress";

const phase = ref<AppUpdateGatePhase>("checking");
const version = ref<string | null>(null);
const downloaded = ref(0);
const total = ref<number | null>(null);
const relaunchFailed = ref(false);
const gatePassed = ref(false);

export function useAppUpdater() {
  async function restartNow() {
    relaunchFailed.value = false;
    try {
      await relaunch();
    } catch (error) {
      console.warn("Failed to relaunch after update", error);
      relaunchFailed.value = true;
    }
  }

  /**
   * Checks for and installs an update before the app opens. Updates are
   * mandatory: the app stays closed until a found update is installed, and only
   * opens without one when there is none or the check itself failed. When
   * installing needs the user's password, stops at `needsPassword` until the
   * user clicks Update, which runs the gate again with the prompt allowed.
   */
  async function runUpdateGate(allowPasswordPrompt = false) {
    phase.value = "checking";
    version.value = null;
    downloaded.value = 0;
    total.value = null;
    relaunchFailed.value = false;

    if (import.meta.dev) {
      gatePassed.value = true;
      return;
    }

    if (!(await waitForTauri())) {
      gatePassed.value = true;
      return;
    }

    const unlistenFns: UnlistenFn[] = [];
    try {
      unlistenFns.push(
        await listen<AppUpdateAvailablePayload>(
          APP_UPDATE_AVAILABLE_EVENT,
          (event) => {
            phase.value = "downloading";
            version.value = event.payload.version;
          },
        ),
      );
      unlistenFns.push(
        await listen<AppUpdateProgressPayload>(
          APP_UPDATE_PROGRESS_EVENT,
          (event) => {
            downloaded.value = event.payload.downloaded;
            total.value = event.payload.total;
          },
        ),
      );

      const result = await invoke<AppUpdateCheckResult>(
        "check_and_install_app_update",
        { allowPasswordPrompt },
      );
      if (result.status === "needsPassword") {
        version.value = result.version;
        phase.value = "needsPassword";
        return;
      }
      if (result.status === "installFailed") {
        version.value = result.version;
        phase.value = "failed";
        return;
      }
      if (result.status === "installed") {
        if (result.version) {
          version.value = result.version;
        }
        phase.value = "restarting";
        await restartNow();
        return;
      }

      gatePassed.value = true;
    } catch (error) {
      console.warn("App update check failed", error);
      gatePassed.value = true;
    } finally {
      unlistenFns.forEach((unlisten) => unlisten());
    }
  }

  /**
   * Checks for and installs an update without the startup gate, so the app
   * stays mounted. Restarts when an update was installed. Call it from a click:
   * it may ask for the user's password.
   */
  async function installUpdateNow(): Promise<AppUpdateCheckResult["status"]> {
    const result = await invoke<AppUpdateCheckResult>(
      "check_and_install_app_update",
      { allowPasswordPrompt: true },
    );
    if (result.status === "installed") {
      await restartNow();
    }
    return result.status;
  }

  async function quit() {
    try {
      await exit(0);
    } catch (error) {
      console.warn("Failed to quit from the update gate", error);
    }
  }

  return {
    phase,
    version,
    downloaded,
    total,
    relaunchFailed,
    gatePassed,
    runUpdateGate,
    installUpdateNow,
    restartNow,
    quit,
  };
}
