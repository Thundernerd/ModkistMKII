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
    await refreshAuthStatus();
  }

  return {
    authStatus,
    refreshAuthStatus,
    logout,
  };
}
