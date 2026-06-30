import { invoke } from "~/utils/tauri";

export type SideloadSourceType = "dll" | "archive";

export interface SideloadedEntry {
  id: string;
  name: string;
  sourceType: SideloadSourceType;
  addedAt?: string;
}

export function useSideload() {
  const entries = ref<SideloadedEntry[]>([]);
  const loading = ref(false);
  const adding = ref(false);
  const removingIds = ref<Set<string>>(new Set());
  const error = ref("");

  async function refreshSideloaded() {
    loading.value = true;
    error.value = "";

    try {
      entries.value = await invoke<SideloadedEntry[]>("list_sideloaded_mods");
    } catch (err) {
      error.value = String(err);
      throw err;
    } finally {
      loading.value = false;
    }
  }

  async function addSideloaded(sourcePath: string) {
    adding.value = true;
    error.value = "";

    try {
      const entry = await invoke<SideloadedEntry>("add_sideloaded_mod", {
        sourcePath,
      });
      await refreshSideloaded();
      return entry;
    } catch (err) {
      error.value = String(err);
      throw err;
    } finally {
      adding.value = false;
    }
  }

  function isRemoving(entryId: string) {
    return removingIds.value.has(entryId);
  }

  async function removeSideloaded(entryId: string) {
    removingIds.value = new Set(removingIds.value).add(entryId);
    error.value = "";

    try {
      entries.value = await invoke<SideloadedEntry[]>("remove_sideloaded_mod", {
        entryId,
      });
    } catch (err) {
      error.value = String(err);
      throw err;
    } finally {
      const next = new Set(removingIds.value);
      next.delete(entryId);
      removingIds.value = next;
    }
  }

  return {
    entries,
    loading,
    adding,
    removingIds,
    error,
    refreshSideloaded,
    addSideloaded,
    removeSideloaded,
    isRemoving,
  };
}
