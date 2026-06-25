import { invoke } from "~/utils/tauri";

export interface GameRunningStatus {
  running: boolean;
  message: string | null;
}

const POLL_INTERVAL_MS = 3000;

const gameRunning = ref(false);
const gameRunningMessage = ref<string | null>(null);

let pollTimer: ReturnType<typeof setInterval> | null = null;
let pollingStarted = false;

export async function refreshGameRunning() {
  try {
    const status = await invoke<GameRunningStatus>("game_running_status");
    gameRunning.value = status.running;
    gameRunningMessage.value = status.message;
  } catch {
    gameRunning.value = false;
    gameRunningMessage.value = null;
  }
}

export function startGameProcessPolling() {
  if (pollingStarted) {
    return;
  }

  pollingStarted = true;
  void refreshGameRunning();
  pollTimer = setInterval(() => {
    void refreshGameRunning();
  }, POLL_INTERVAL_MS);
}

export function stopGameProcessPolling() {
  if (!pollingStarted) {
    return;
  }

  pollingStarted = false;
  if (pollTimer) {
    clearInterval(pollTimer);
    pollTimer = null;
  }
}

export function useGameProcess() {
  return {
    gameRunning,
    gameRunningMessage,
    refreshGameRunning,
    startGameProcessPolling,
    stopGameProcessPolling,
  };
}
