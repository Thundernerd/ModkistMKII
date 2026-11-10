import { invoke } from "~/utils/tauri";

export interface AppSettings {
  autoUpdateMods: boolean;
  skipSignIn: boolean;
  ignoreBepInExVersionWarning: boolean;
  customWinePrefixPath: string | null;
}

const autoUpdateMods = ref(true);
const skipSignIn = ref(false);
const ignoreBepInExVersionWarning = ref(false);
const customWinePrefixPath = ref<string | null>(null);
const settingsReady = ref(false);

function syncSettings(settings: AppSettings) {
  autoUpdateMods.value = settings.autoUpdateMods;
  skipSignIn.value = settings.skipSignIn;
  ignoreBepInExVersionWarning.value = settings.ignoreBepInExVersionWarning;
  customWinePrefixPath.value = settings.customWinePrefixPath ?? null;
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

  async function setCustomWinePrefixPath(path: string) {
    const settings = await invoke<AppSettings>("set_custom_wine_prefix_path", {
      path,
    });
    syncSettings(settings);
    return settings;
  }

  async function clearCustomWinePrefixPath() {
    const settings = await invoke<AppSettings>("clear_custom_wine_prefix_path");
    syncSettings(settings);
    return settings;
  }

  async function rememberSkipSignIn() {
    const settings = await invoke<AppSettings>("remember_skip_sign_in");
    syncSettings(settings);
    return settings;
  }

  return {
    autoUpdateMods,
    skipSignIn,
    ignoreBepInExVersionWarning,
    customWinePrefixPath,
    settingsReady,
    refreshAppSettings,
    setAutoUpdateMods,
    setIgnoreBepInExVersionWarning,
    rememberIgnoreBepInExVersionWarning,
    isBepInExVersionWarningSuppressed,
    setCustomWinePrefixPath,
    clearCustomWinePrefixPath,
    rememberSkipSignIn,
  };
}
