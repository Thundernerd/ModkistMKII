import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { invoke } from "~/utils/tauri";
import { logger } from "~/utils/logger";

export interface ModioStatus {
  configured: boolean;
  message?: string;
  /** mod.io refused the game API key this build embeds; only an update fixes it. */
  apiKeyRefused?: boolean;
}

const API_KEY_REFUSED_EVENT = "modio-api-key-refused";

const configured = ref(false);
const apiKeyRefused = ref(false);
let unlistenApiKeyRefused: UnlistenFn | undefined;
const message = ref("");
const checked = ref(false);
let checkInFlight: Promise<void> | null = null;

export async function refreshModioStatus() {
  if (checkInFlight) {
    return checkInFlight;
  }

  checkInFlight = (async () => {
    try {
      const status = await invoke<ModioStatus>("modio_status");
      configured.value = status.configured;
      message.value = status.message ?? "";
      apiKeyRefused.value = status.apiKeyRefused === true;
    } catch {
      configured.value = false;
      message.value = "";
    } finally {
      checked.value = true;
      checkInFlight = null;
    }
  })();

  return checkInFlight;
}

/** Starts listening once; the backend emits this when mod.io refuses the game API key. */
export async function listenForApiKeyRefusal() {
  if (unlistenApiKeyRefused) return;
  unlistenApiKeyRefused = await listen(API_KEY_REFUSED_EVENT, () => {
    logger.info("mod.io refused the game API key");
    apiKeyRefused.value = true;
  });
  // A refusal can arrive before this listener exists.
  await refreshModioStatus();
}

export function useModioStatus() {
  return {
    modioConfigured: configured,
    modioMessage: message,
    modioStatusChecked: checked,
    modioApiKeyRefused: apiKeyRefused,
    refreshModioStatus,
    listenForApiKeyRefusal,
  };
}
