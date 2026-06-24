const host = process.env.TAURI_DEV_HOST;

export default defineNuxtConfig({
  compatibilityDate: "2025-05-15",
  ssr: false,
  spaLoadingTemplate: "./spa-loading-template.html",
  telemetry: false,
  devtools: { enabled: false },
  css: ["~/assets/css/main.css"],
  devServer: {
    host: host || "localhost",
    port: 1420,
  },
  vite: {
    clearScreen: false,
    envPrefix: ["VITE_", "TAURI_"],
    optimizeDeps: {
      include: ["@tauri-apps/api/core", "@tauri-apps/plugin-dialog"],
    },
    server: {
      strictPort: true,
      hmr: host
        ? {
            protocol: "ws",
            host,
            port: 1421,
          }
        : undefined,
      watch: {
        ignored: ["**/src-tauri/**"],
      },
    },
    build: {
      target:
        process.env.TAURI_ENV_PLATFORM === "windows"
          ? "chrome105"
          : "safari13",
      minify: !process.env.TAURI_ENV_DEBUG ? "esbuild" : false,
      sourcemap: !!process.env.TAURI_ENV_DEBUG,
    },
  },
  ignore: ["**/src-tauri/**"],
});
