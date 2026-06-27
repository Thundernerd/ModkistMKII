import { invoke } from "~/utils/tauri";
import { revealItemInDir } from "@tauri-apps/plugin-opener";

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

  async function openGameFolder() {
    await refreshGamePathStatus();
    const status = gamePathStatus.value;

    if (!status.valid || !status.path) {
      throw new Error(
        status.message ?? "Select a valid Zeepkist game directory first.",
      );
    }

    await revealItemInDir(status.path);
  }

  return {
    gamePathStatus,
    refreshGamePathStatus,
    setGamePath,
    detectGamePaths,
    openGameFolder,
  };
}
