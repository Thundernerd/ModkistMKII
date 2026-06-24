function isNuxtMounted(): boolean {
  const root = document.getElementById("__nuxt");
  return !!root && root.childElementCount > 0;
}

export default defineNuxtPlugin({
  name: "tauri",
  enforce: "pre",
  setup() {
    if (!import.meta.env.TAURI_ENV_PLATFORM) return;
    if (isNuxtMounted()) return;

    let reloaded = false;

    const reloadIfBlank = () => {
      if (reloaded || isNuxtMounted()) return;
      reloaded = true;
      window.location.reload();
    };

    const timeouts = [400, 1200].map((delay) =>
      window.setTimeout(reloadIfBlank, delay),
    );

    const observer = new MutationObserver(() => {
      if (!isNuxtMounted()) return;
      timeouts.forEach((id) => window.clearTimeout(id));
      observer.disconnect();
    });

    observer.observe(document.body, { childList: true, subtree: true });
  },
});
