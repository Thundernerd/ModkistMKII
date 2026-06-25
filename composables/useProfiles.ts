import { cancelSubscriptionSync } from "~/composables/useModInstall";
import { invoke } from "~/utils/tauri";
import { logger } from "~/utils/logger";

export type ProfileKind = "vanilla" | "user" | "custom";

export interface ProfileSummary {
  id: string;
  name: string;
  kind: ProfileKind;
  installBlocked: boolean;
  isActive: boolean;
  selectable: boolean;
}

export interface ActiveProfileInfo {
  id: string;
  name: string;
  kind: ProfileKind;
  installBlocked: boolean;
}

export interface ProfileListResult {
  profiles: ProfileSummary[];
  activeProfileId: string;
}

const profiles = ref<ProfileSummary[]>([]);
const activeProfileId = ref("vanilla");
const activeProfile = ref<ActiveProfileInfo | null>(null);
const loading = ref(false);
const switching = ref(false);
const error = ref("");

const installBlocked = computed(
  () => activeProfile.value?.installBlocked ?? false,
);

const isUserProfileActive = computed(
  () => activeProfile.value?.kind === "user",
);

function syncFromList(result: ProfileListResult) {
  profiles.value = result.profiles;
  activeProfileId.value = result.activeProfileId;
  activeProfile.value =
    result.profiles.find((profile) => profile.isActive) ?? null;
}

export function useProfiles() {
  async function refreshProfiles() {
    loading.value = true;
    error.value = "";
    try {
      const result = await invoke<ProfileListResult>("list_profiles");
      syncFromList(result);
    } catch (err) {
      error.value = err instanceof Error ? err.message : String(err);
      throw err;
    } finally {
      loading.value = false;
    }
  }

  async function switchProfile(profileId: string) {
    if (profileId === activeProfileId.value) {
      return activeProfile.value;
    }

    switching.value = true;
    error.value = "";
    await cancelSubscriptionSync();
    try {
      logger.info(`Switching profile to ${profileId}`);
      const next = await invoke<ActiveProfileInfo>("switch_profile", {
        profileId,
      });
      activeProfile.value = next;
      activeProfileId.value = next.id;
      await refreshProfiles();
      logger.info(`Active profile is now ${next.name} (${next.kind})`);
      return next;
    } catch (err) {
      error.value = err instanceof Error ? err.message : String(err);
      throw err;
    } finally {
      switching.value = false;
    }
  }

  async function createProfile(name: string) {
    error.value = "";
    const profile = await invoke<ProfileSummary>("create_profile", { name });
    await refreshProfiles();
    await switchProfile(profile.id);
    return profile;
  }

  async function deleteProfile(profileId: string) {
    error.value = "";
    await invoke("delete_profile", { profileId });
    await refreshProfiles();
  }

  async function renameProfile(profileId: string, name: string) {
    error.value = "";
    const profile = await invoke<ProfileSummary>("rename_profile", {
      profileId,
      name,
    });
    await refreshProfiles();
    return profile;
  }

  async function checkLogoutRequiresProfileSelection() {
    return invoke<boolean>("logout_requires_profile_selection_command");
  }

  function logoutPickerProfiles() {
    return profiles.value.filter((profile) => profile.kind !== "user");
  }

  return {
    profiles,
    activeProfile,
    activeProfileId,
    loading,
    switching,
    error,
    installBlocked,
    isUserProfileActive,
    refreshProfiles,
    switchProfile,
    createProfile,
    deleteProfile,
    renameProfile,
    checkLogoutRequiresProfileSelection,
    logoutPickerProfiles,
  };
}
