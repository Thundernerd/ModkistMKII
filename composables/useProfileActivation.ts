import type { ProfileSummary } from "~/composables/useProfiles";
import { useModioErrorFeedback } from "~/composables/useModioErrorFeedback";
import { useProfiles } from "~/composables/useProfiles";
import { useModInstall } from "~/composables/useModInstall";
import { useProfileSwitchUi } from "~/composables/useProfileSwitchUi";
import { useNotifications } from "~/composables/useNotifications";

export function useProfileActivation() {
  const { switchProfile } = useProfiles();
  const { notifyIfRateLimited } = useModioErrorFeedback();
  const { pushNotification } = useNotifications();
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
      if (profile.kind === "user" || profile.kind === "custom") {
        resetSessionSync();
        setProfileSwitchMessage(
          profile.kind === "user"
            ? "Syncing subscribed mods…"
            : "Restoring profile mods…",
        );
        await syncSubscribedModsIfNeeded();
      }
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      if (!notifyIfRateLimited(message)) {
        pushNotification({
          title: "Profile switch failed",
          message,
          tone: "error",
          durationMs: 10_000,
        });
      }
      throw error;
    } finally {
      endProfileSwitch();
    }
  }

  async function refreshActiveProfileMods(profile: ProfileSummary) {
    await refreshInstalled({ force: true });
    if (profile.kind === "user" || profile.kind === "custom") {
      resetSessionSync();
      await syncSubscribedModsIfNeeded();
    }
  }

  return {
    activateProfile,
    refreshActiveProfileMods,
  };
}
