import { invoke } from "~/utils/tauri";
import { refreshGameRunning } from "~/composables/useGameProcess";

const launching = ref(false);
const launchError = ref<string | null>(null);

export async function launchGame() {
  if (launching.value) {
    return;
  }

  launching.value = true;
  launchError.value = null;

  try {
    await invoke("launch_game");
    await refreshGameRunning();
  } catch (error) {
    launchError.value =
      error instanceof Error ? error.message : String(error);
    throw error;
  } finally {
    launching.value = false;
  }
}

export function clearLaunchError() {
  launchError.value = null;
}

export function useGameLaunch() {
  return {
    launching,
    launchError,
    launchGame,
    clearLaunchError,
  };
}
