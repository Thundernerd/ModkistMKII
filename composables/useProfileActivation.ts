import type { ProfileSummary } from "~/composables/useProfiles";
import { useProfiles } from "~/composables/useProfiles";
import { useModInstall } from "~/composables/useModInstall";
import { useProfileSwitchUi } from "~/composables/useProfileSwitchUi";

export function useProfileActivation() {
  const { switchProfile } = useProfiles();
  const {
    invalidateInstalledModsCache,
    resetSessionSync,
    refreshInstalled,
    syncSubscribedModsIfNeeded,
  } = useModInstall();
  const {
    beginProfileSwitch,
    setProfileSwitchMessage,
    endProfileSwitch,
  } = useProfileSwitchUi();

  async function activateProfile(profile: ProfileSummary) {
    beginProfileSwitch(profile.name);

    try {
      setProfileSwitchMessage(`Switching to ${profile.name}…`);
      await switchProfile(profile.id);
      invalidateInstalledModsCache();
      setProfileSwitchMessage("Loading installed mods…");
      await refreshInstalled({ force: true });
      if (profile.kind === "user") {
        resetSessionSync();
        setProfileSwitchMessage("Syncing subscribed mods…");
        await syncSubscribedModsIfNeeded();
      }
    } finally {
      endProfileSwitch();
    }
  }

  async function refreshActiveProfileMods(profile: ProfileSummary) {
    await refreshInstalled({ force: true });
    if (profile.kind === "user") {
      resetSessionSync();
      await syncSubscribedModsIfNeeded();
    }
  }

  return {
    activateProfile,
    refreshActiveProfileMods,
  };
}
