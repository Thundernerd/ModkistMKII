import { relaunch } from "@tauri-apps/plugin-process";
import { invoke, waitForTauri } from "~/utils/tauri";

export interface AppUpdateCheckResult {
  status: "skipped" | "upToDate" | "installed" | string;
  version: string | null;
}

export function useAppUpdater() {
  const { autoUpdateApp, refreshAppSettings } = useAppSettings();
  const { pushNotification } = useNotifications();

  async function checkAndInstallAppUpdate() {
    if (import.meta.dev) {
      return;
    }

    if (!(await waitForTauri())) {
      return;
    }

    try {
      await refreshAppSettings();
      if (!autoUpdateApp.value) {
        return;
      }

      const result = await invoke<AppUpdateCheckResult>(
        "check_and_install_app_update",
      );
      if (result.status !== "installed" || !result.version) {
        return;
      }

      pushNotification({
        title: "Update ready",
        message: `Modkist v${result.version} was installed. Restart to finish updating.`,
        tone: "success",
        durationMs: 0,
        action: {
          label: "Restart",
          onClick: () => relaunch(),
        },
      });
    } catch (error) {
      console.warn("App update check failed", error);
    }
  }

  return { checkAndInstallAppUpdate };
}
