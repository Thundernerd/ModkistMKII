import { invoke } from "~/utils/tauri";

export type BepInExState = "missing" | "installed" | "wrongVersion";

export interface BepInExStatus {
  state: BepInExState;
  foundVersion?: string;
  message?: string;
  canContinue: boolean;
}

export function useBepInEx() {
  const bepinexStatus = ref<BepInExStatus>({
    state: "missing",
    canContinue: false,
  });
  const loading = ref(false);
  const installing = ref(false);
  const error = ref("");

  async function refreshBepInExStatus() {
    loading.value = true;
    error.value = "";

    try {
      bepinexStatus.value = await invoke<BepInExStatus>("bepinex_status");
    } catch (err) {
      error.value = String(err);
    } finally {
      loading.value = false;
    }
  }

  async function installBepInEx() {
    installing.value = true;
    error.value = "";

    try {
      bepinexStatus.value = await invoke<BepInExStatus>("install_bepinex");
    } catch (err) {
      error.value = String(err);
      throw err;
    } finally {
      installing.value = false;
    }
  }

  return {
    bepinexStatus,
    loading,
    installing,
    error,
    refreshBepInExStatus,
    installBepInEx,
  };
}
