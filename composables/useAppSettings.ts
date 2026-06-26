import { invoke } from "~/utils/tauri";

export interface AppSettings {
  autoUpdateMods: boolean;
}

const autoUpdateMods = ref(true);
const settingsReady = ref(false);

export function useAppSettings() {
  async function refreshAppSettings() {
    const settings = await invoke<AppSettings>("get_app_settings");
    autoUpdateMods.value = settings.autoUpdateMods;
    settingsReady.value = true;
    return settings;
  }

  async function setAutoUpdateMods(enabled: boolean) {
    const settings = await invoke<AppSettings>("set_auto_update_mods", {
      enabled,
    });
    autoUpdateMods.value = settings.autoUpdateMods;
    return settings;
  }

  return {
    autoUpdateMods,
    settingsReady,
    refreshAppSettings,
    setAutoUpdateMods,
  };
}
