import { invoke } from "~/utils/tauri";

export interface GamePathStatus {
  configured: boolean;
  valid: boolean;
  path?: string;
  message?: string;
}

export interface GamePathCandidate {
  path: string;
  source: string;
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

  async function detectGamePaths() {
    return invoke<GamePathCandidate[]>("detect_game_paths_command");
  }

  return {
    gamePathStatus,
    refreshGamePathStatus,
    setGamePath,
    detectGamePaths,
  };
}
