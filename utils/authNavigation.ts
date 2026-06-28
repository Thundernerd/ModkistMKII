import { invoke } from "~/utils/tauri";
import type { AuthStatus } from "~/composables/useModioAuth";
import type { BepInExStatus } from "~/composables/useBepInEx";
import type { GamePathStatus } from "~/composables/useGamePath";

const AUTH_REQUIRED_PATHS = new Set(["/user"]);

function setupRoute(redirect?: string) {
  return redirect ? { path: "/setup", query: { redirect } } : "/setup";
}

function bepinexRoute(redirect?: string) {
  return redirect ? { path: "/bepinex", query: { redirect } } : "/bepinex";
}

async function resolveDestination(redirect?: string): Promise<string> {
  const destination =
    redirect && redirect.startsWith("/") ? redirect : "/home";

  if (!AUTH_REQUIRED_PATHS.has(destination)) {
    return destination;
  }

  const auth = await invoke<AuthStatus>("auth_status");
  return auth.loggedIn ? destination : "/home";
}

async function needsBepInExOnboarding(status: BepInExStatus) {
  return status.state === "missing";
}

export function readRedirectParam(value: unknown): string | undefined {
  const redirect = Array.isArray(value) ? value[0] : value;
  if (typeof redirect === "string" && redirect.startsWith("/")) {
    return redirect;
  }
  return undefined;
}

export async function navigateToApp(redirect?: string) {
  const gamePath = await invoke<GamePathStatus>("game_path_status");
  if (!gamePath.valid) {
    await navigateTo(setupRoute(redirect));
    return;
  }

  const bepinex = await invoke<BepInExStatus>("bepinex_status");
  if (await needsBepInExOnboarding(bepinex)) {
    await navigateTo(bepinexRoute(redirect));
    return;
  }

  await navigateTo(await resolveDestination(redirect));
}

export async function continuePastBepInEx(redirect?: string) {
  await navigateTo(await resolveDestination(redirect));
}

export async function ensureGamePath(redirect?: string): Promise<boolean> {
  const gamePath = await invoke<GamePathStatus>("game_path_status");
  if (!gamePath.valid) {
    await navigateTo(setupRoute(redirect));
    return false;
  }

  return true;
}

export async function ensureBepInEx(redirect?: string): Promise<boolean> {
  const bepinex = await invoke<BepInExStatus>("bepinex_status");
  if (bepinex.state === "missing") {
    await navigateTo(bepinexRoute(redirect));
    return false;
  }

  return true;
}
