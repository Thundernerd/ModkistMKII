import { invoke as tauriInvoke } from "@tauri-apps/api/core";

type TauriInternals = {
  invoke: typeof tauriInvoke;
};

function getTauriInternals(): TauriInternals | undefined {
  return (globalThis as { __TAURI_INTERNALS__?: TauriInternals })
    .__TAURI_INTERNALS__;
}

export function isTauriReady(): boolean {
  return getTauriInternals()?.invoke !== undefined;
}

export async function waitForTauri(timeoutMs = 5000): Promise<boolean> {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    if (isTauriReady()) return true;
    await new Promise((resolve) => setTimeout(resolve, 25));
  }
  return isTauriReady();
}

export async function invoke<T>(
  cmd: string,
  args: Record<string, unknown> = {},
): Promise<T> {
  await waitForTauri();
  const internals = getTauriInternals();
  if (!internals) {
    throw new Error("Tauri IPC is not available");
  }
  return internals.invoke<T>(cmd, args);
}
