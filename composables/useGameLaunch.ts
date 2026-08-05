import { invoke } from "~/utils/tauri";
import { refreshGameRunning } from "~/composables/useGameProcess";

const LAUNCH_OVERLAY_MS = 10_000;

const launching = ref(false);
const launchError = ref<string | null>(null);
const launchOverlayActive = ref(false);

let overlayTimer: ReturnType<typeof setTimeout> | null = null;

function hideLaunchOverlay() {
  if (overlayTimer !== null) {
    clearTimeout(overlayTimer);
    overlayTimer = null;
  }
  launchOverlayActive.value = false;
}

function showLaunchOverlay() {
  hideLaunchOverlay();
  launchOverlayActive.value = true;
  overlayTimer = setTimeout(() => {
    overlayTimer = null;
    launchOverlayActive.value = false;
  }, LAUNCH_OVERLAY_MS);
}

export async function launchGame() {
  if (launching.value) {
    return;
  }

  launching.value = true;
  launchError.value = null;
  showLaunchOverlay();

  try {
    await invoke("launch_game");
    await refreshGameRunning();
  } catch (error) {
    hideLaunchOverlay();
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
    launchOverlayActive,
    launchGame,
    clearLaunchError,
  };
}
