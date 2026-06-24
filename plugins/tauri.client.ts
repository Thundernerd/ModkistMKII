import { isTauriReady } from "~/utils/tauri";

export default defineNuxtPlugin({
  name: "tauri",
  enforce: "pre",
  setup() {
    if (!import.meta.dev || !import.meta.env.TAURI_ENV_PLATFORM) return;
    if (isTauriReady()) return;

    const timeout = window.setTimeout(() => {
      const root = document.getElementById("__nuxt");
      if (root && root.childElementCount === 0) {
        window.location.reload();
      }
    }, 200);

    const observer = new MutationObserver(() => {
      const root = document.getElementById("__nuxt");
      if (!root || root.childElementCount === 0) return;
      window.clearTimeout(timeout);
      observer.disconnect();
    });

    observer.observe(document.body, { childList: true, subtree: true });
  },
});
