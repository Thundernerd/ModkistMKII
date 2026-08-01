import { invoke } from "~/utils/tauri";
import {
  wineWinhttpFeedback,
  type WineWinhttpStatus,
} from "~/utils/wineWinhttp";

const wineStatus = ref<WineWinhttpStatus | null>(null);
const wineChecking = ref(false);
const wineError = ref("");

export function useWineWinhttp() {
  const wineFeedback = computed(() => wineWinhttpFeedback(wineStatus.value));

  const needsWineAttention = computed(() => {
    const state = wineStatus.value?.state;
    return state === "notFound" || state === "failed";
  });

  function syncWineStatus(status?: WineWinhttpStatus | null) {
    if (status === undefined) {
      return;
    }
    wineStatus.value = status;
    wineError.value = "";
  }

  async function configureWineWinhttp() {
    wineChecking.value = true;
    wineError.value = "";

    try {
      wineStatus.value = await invoke<WineWinhttpStatus>("configure_wine_winhttp");
      return wineStatus.value;
    } catch (err) {
      wineError.value = String(err);
      throw err;
    } finally {
      wineChecking.value = false;
    }
  }

  return {
    wineStatus,
    wineChecking,
    wineError,
    wineFeedback,
    needsWineAttention,
    syncWineStatus,
    configureWineWinhttp,
  };
}
