import { invoke } from "~/utils/tauri";
import type { ModListResult } from "~/composables/useMods";

export interface UserProfile {
  username: string;
  profileUrl: string;
  avatarUrl?: string;
}

export function useUserProfile() {
  const profile = ref<UserProfile | null>(null);
  const userMods = ref<ModListResult>({ mods: [], total: 0 });
  const loading = ref(false);
  const error = ref("");

  async function fetchUserData() {
    loading.value = true;
    error.value = "";

    try {
      const [profileResult, modsResult] = await Promise.all([
        invoke<UserProfile>("get_user_profile"),
        invoke<ModListResult>("list_user_mods"),
      ]);
      profile.value = profileResult;
      userMods.value = modsResult;
    } catch (err) {
      error.value = String(err);
      profile.value = null;
      userMods.value = { mods: [], total: 0 };
    } finally {
      loading.value = false;
    }
  }

  return {
    profile,
    userMods,
    loading,
    error,
    fetchUserData,
  };
}
