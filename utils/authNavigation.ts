import { invoke } from "~/utils/tauri";
import type { GamePathStatus } from "~/composables/useGamePath";

export async function navigateToApp() {
  const gamePath = await invoke<GamePathStatus>("game_path_status");
  if (!gamePath.valid) {
    await navigateTo("/setup");
    return;
  }

  await navigateTo("/home");
}

export async function ensureGamePath(): Promise<boolean> {
  const gamePath = await invoke<GamePathStatus>("game_path_status");
  if (!gamePath.valid) {
    await navigateTo("/setup");
    return false;
  }

  return true;
}
