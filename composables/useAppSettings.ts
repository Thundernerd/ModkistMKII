import { invoke } from "~/utils/tauri";

export interface AppSettings {
  autoUpdateMods: boolean;
  autoUpdateApp: boolean;
  skipSignIn: boolean;
  ignoreBepInExVersionWarning: boolean;
}

const autoUpdateMods = ref(true);
const autoUpdateApp = ref(true);
const skipSignIn = ref(false);
const ignoreBepInExVersionWarning = ref(false);
const settingsReady = ref(false);

function syncSettings(settings: AppSettings) {
  autoUpdateMods.value = settings.autoUpdateMods;
  autoUpdateApp.value = settings.autoUpdateApp;
  skipSignIn.value = settings.skipSignIn;
  ignoreBepInExVersionWarning.value = settings.ignoreBepInExVersionWarning;
}

export function useAppSettings() {
  async function refreshAppSettings() {
    const settings = await invoke<AppSettings>("get_app_settings");
    syncSettings(settings);
    settingsReady.value = true;
    return settings;
  }

  async function setAutoUpdateMods(enabled: boolean) {
    const settings = await invoke<AppSettings>("set_auto_update_mods", {
      enabled,
    });
    syncSettings(settings);
    return settings;
  }

  async function setAutoUpdateApp(enabled: boolean) {
    const settings = await invoke<AppSettings>("set_auto_update_app", {
      enabled,
    });
    syncSettings(settings);
    return settings;
  }

  async function setIgnoreBepInExVersionWarning(enabled: boolean) {
    const settings = await invoke<AppSettings>("set_ignore_bepinex_version_warning", {
      enabled,
    });
    syncSettings(settings);
    return settings;
  }

  async function rememberIgnoreBepInExVersionWarning() {
    const settings = await invoke<AppSettings>(
      "remember_ignore_bepinex_version_warning",
    );
    syncSettings(settings);
    return settings;
  }

  async function isBepInExVersionWarningSuppressed() {
    return invoke<boolean>("get_ignore_bepinex_version_warning_enabled");
  }

  async function rememberSkipSignIn() {
    const settings = await invoke<AppSettings>("remember_skip_sign_in");
    syncSettings(settings);
    return settings;
  }

  return {
    autoUpdateMods,
    autoUpdateApp,
    skipSignIn,
    ignoreBepInExVersionWarning,
    settingsReady,
    refreshAppSettings,
    setAutoUpdateMods,
    setAutoUpdateApp,
    setIgnoreBepInExVersionWarning,
    rememberIgnoreBepInExVersionWarning,
    isBepInExVersionWarningSuppressed,
    rememberSkipSignIn,
  };
}
