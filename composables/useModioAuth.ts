import { invoke } from "~/utils/tauri";

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
    await invoke("logout");
    const { resetSessionSync } = useModInstall();
    const { refreshProfiles } = useProfiles();
    resetSessionSync();
    await refreshAuthStatus();
    await refreshProfiles();
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
