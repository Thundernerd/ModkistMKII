import { invoke } from "~/utils/tauri";
import { logger } from "~/utils/logger";

export interface AuthStatus {
  loggedIn: boolean;
  username?: string;
}

export interface AuthUser {
  username: string;
  profileUrl: string;
}

const authStatus = ref<AuthStatus>({ loggedIn: false });

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
      logger.debug("Could not refresh profiles after logout", err);
    });
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
    logout,
    checkLogoutRequiresProfileSelection,
    completeLogout,
  };
}
