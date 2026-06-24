import { invoke } from "~/utils/tauri";

export interface GamePathStatus {
  configured: boolean;
  valid: boolean;
  path?: string;
  message?: string;
}

const gamePathStatus = ref<GamePathStatus>({
  configured: false,
  valid: false,
});

export function useGamePath() {
  async function refreshGamePathStatus() {
    gamePathStatus.value = await invoke<GamePathStatus>("game_path_status");
  }

  async function setGamePath(path: string) {
    gamePathStatus.value = await invoke<GamePathStatus>("set_game_path", {
      path,
    });
  }

  return {
    gamePathStatus,
    refreshGamePathStatus,
    setGamePath,
  };
}
