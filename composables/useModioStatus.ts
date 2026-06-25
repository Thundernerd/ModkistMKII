import { invoke } from "~/utils/tauri";

export interface ModioStatus {
  configured: boolean;
  message?: string;
}

const configured = ref(false);
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

export function useModioStatus() {
  return {
    modioConfigured: configured,
    modioMessage: message,
    modioStatusChecked: checked,
    refreshModioStatus,
  };
}
