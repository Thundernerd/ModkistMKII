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
const message = ref("");
const checked = ref(false);
/** Only ever turns on during a run: mod.io does not un-refuse a key. */
const apiKeyRefused = ref(false);
let checkInFlight: Promise<void> | null = null;
let unlistenApiKeyRefused: UnlistenFn | undefined;

export async function refreshModioStatus() {
  if (checkInFlight) {
    return checkInFlight;
  }

  checkInFlight = (async () => {
    try {
      const status = await invoke<ModioStatus>("modio_status");
      configured.value = status.configured;
      message.value = status.message ?? "";
      // A status call that started before the refusal must not hide the banner.
      apiKeyRefused.value ||= status.apiKeyRefused === true;
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
  // A refusal can arrive before this listener exists. Wait out a check that
  // may have started before it, then ask again.
  await checkInFlight;
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
