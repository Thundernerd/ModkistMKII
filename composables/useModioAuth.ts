import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { invoke } from "~/utils/tauri";
import { logger } from "~/utils/logger";

export interface AuthStatus {
  loggedIn: boolean;
  username?: string;
  /** mod.io rejected the stored token and the backend signed the user out. */
  sessionExpired?: boolean;
}

export interface AuthUser {
  username: string;
  profileUrl: string;
}

const SESSION_EXPIRED_EVENT = "modio-session-expired";

const authStatus = ref<AuthStatus>({ loggedIn: false });
let unlistenSessionExpired: UnlistenFn | undefined;

export function useModioAuth() {
  async function refreshAuthStatus() {
    authStatus.value = await invoke<AuthStatus>("auth_status");
  }

  async function logout() {
    logger.info("Logging out");
    const { resetSessionSync, cancelSubscriptionSync } = useModInstall();
    const { refreshProfiles } = useProfiles();
    await cancelSubscriptionSync().catch(() => {});
    await invoke("logout");
    resetSessionSync();
    await refreshAuthStatus();
    await refreshProfiles().catch((err) => {
      logger.debug("Did not refresh profiles after logout", err);
    });
  }

  async function handleSessionExpired() {
    logger.info("mod.io session expired");
    const { resetSessionSync } = useModInstall();
    const { refreshProfiles } = useProfiles();
    resetSessionSync();
    await refreshAuthStatus();
    await refreshProfiles().catch((err) => {
      logger.debug("Did not refresh profiles after session expired", err);
    });
  }

  /** Starts listening once; the backend emits this when mod.io rejects the token. */
  async function listenForSessionExpiry() {
    if (unlistenSessionExpired) return;
    unlistenSessionExpired = await listen(SESSION_EXPIRED_EVENT, () => {
      handleSessionExpired().catch((err) => {
        logger.debug("Did not handle expired session", err);
      });
    });
    // The startup token check can finish before this listener exists.
    await refreshAuthStatus();
  }

  async function checkLogoutRequiresProfileSelection() {
    return invoke<boolean>("logout_requires_profile_selection_command");
  }

  async function completeLogout() {
    await logout();
  }

  return {
    authStatus,
    refreshAuthStatus,
    listenForSessionExpiry,
    logout,
    checkLogoutRequiresProfileSelection,
    completeLogout,
  };
}
