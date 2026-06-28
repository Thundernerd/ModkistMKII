import { invoke } from "~/utils/tauri";

export interface FailedSyncModEntry {
  modId: number;
  ignored: boolean;
}

export interface FailedSyncModList {
  mods: FailedSyncModEntry[];
}

export function useFailedSyncMods() {
  const mods = ref<FailedSyncModEntry[]>([]);
  const loading = ref(false);
  const error = ref("");

  async function refreshFailedSyncMods() {
    loading.value = true;
    error.value = "";
    try {
      const result = await invoke<FailedSyncModList>("list_failed_sync_mods_command");
      mods.value = result.mods;
    } catch (err) {
      error.value = err instanceof Error ? err.message : String(err);
      mods.value = [];
    } finally {
      loading.value = false;
    }
  }

  async function setIgnored(modId: number, ignored: boolean) {
    error.value = "";
    try {
      const result = await invoke<FailedSyncModList>("set_failed_sync_mod_ignored", {
        modId,
        ignored,
      });
      mods.value = result.mods;
    } catch (err) {
      error.value = err instanceof Error ? err.message : String(err);
      throw err;
    }
  }

  async function unsubscribe(modId: number) {
    error.value = "";
    try {
      const result = await invoke<FailedSyncModList>("unsubscribe_failed_sync_mod", {
        modId,
      });
      mods.value = result.mods;
    } catch (err) {
      error.value = err instanceof Error ? err.message : String(err);
      throw err;
    }
  }

  return {
    mods,
    loading,
    error,
    refreshFailedSyncMods,
    setIgnored,
    unsubscribe,
  };
}
