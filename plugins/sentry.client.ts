import * as Sentry from "@sentry/vue";

function sentryDsn(): string | undefined {
  const value = import.meta.env.VITE_SENTRY_DSN;
  if (typeof value !== "string") {
    return undefined;
  }

  const trimmed = value.trim();
  return trimmed.length > 0 ? trimmed : undefined;
}

export default defineNuxtPlugin({
  name: "sentry",
  enforce: "pre",
  setup(nuxtApp) {
    const dsn = sentryDsn();
    if (!dsn) {
      return;
    }

    Sentry.init({
      app: nuxtApp.vueApp,
      dsn,
      environment: import.meta.env.DEV ? "development" : "production",
      attachProps: false,
    });
  },
});
