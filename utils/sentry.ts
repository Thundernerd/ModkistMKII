export function sentryEnvironment(): string {
  return import.meta.env.DEV ? "development" : "production";
}
