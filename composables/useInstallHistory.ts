export type InstallHistoryAction = "installed" | "updated";

export interface InstallHistoryEntry {
  id: number;
  modId: number;
  name: string;
  logoUrl: string;
  action: InstallHistoryAction;
  fileId: number | null;
  versionLabel?: string;
  dependencyCount: number;
  occurredAt: number;
}

export interface RecordInstallOptions {
  modId: number;
  name: string;
  logoUrl?: string;
  action: InstallHistoryAction;
  fileId?: number | null;
  versionLabel?: string;
  dependencyCount?: number;
  occurredAt?: number;
}

const MAX_HISTORY_ENTRIES = 50;

const history = ref<InstallHistoryEntry[]>([]);

let nextId = 1;

export function useInstallHistory() {
  function recordInstall(options: RecordInstallOptions) {
    const entry: InstallHistoryEntry = {
      id: nextId++,
      modId: options.modId,
      name: options.name,
      logoUrl: options.logoUrl ?? "",
      action: options.action,
      fileId: options.fileId ?? null,
      versionLabel: options.versionLabel,
      dependencyCount: options.dependencyCount ?? 0,
      occurredAt: options.occurredAt ?? Date.now(),
    };

    history.value = [entry, ...history.value].slice(0, MAX_HISTORY_ENTRIES);
    return entry;
  }

  function clearHistory() {
    history.value = [];
  }

  return {
    history,
    recordInstall,
    clearHistory,
  };
}
