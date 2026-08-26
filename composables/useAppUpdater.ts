import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { relaunch } from "@tauri-apps/plugin-process";
import { invoke, waitForTauri } from "~/utils/tauri";

export type AppUpdateGatePhase = "checking" | "downloading" | "restarting";

export interface AppUpdateCheckResult {
  status: "skipped" | "upToDate" | "installed" | string;
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

  async function runUpdateGate() {
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
      );
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

  return {
    phase,
    version,
    downloaded,
    total,
    relaunchFailed,
    gatePassed,
    runUpdateGate,
    restartNow,
  };
}
