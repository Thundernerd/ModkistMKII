import * as Sentry from "@sentry/vue";
import { defaultOptions } from "tauri-plugin-sentry-api";

import { sentryEnvironment } from "~/utils/sentry";

export default defineNuxtPlugin({
  name: "sentry",
  setup(nuxtApp) {
    if (!import.meta.env.TAURI_ENV_PLATFORM) {
      return;
    }

    if (Sentry.getClient()) {
      return;
    }

    Sentry.init({
      ...defaultOptions,
      app: nuxtApp.vueApp,
      environment: sentryEnvironment(),
      attachProps: false,
      // Benign browser warnings surfaced via window.onerror, not real bugs:
      // https://github.com/WICG/resize-observer/issues/38
      ignoreErrors: [
        "ResizeObserver loop limit exceeded",
        "ResizeObserver loop completed with undelivered notifications.",
      ],
    });
  },
});
