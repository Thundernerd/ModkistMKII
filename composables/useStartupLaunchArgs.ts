import { launchGame } from "~/composables/useGameLaunch";
import { useModInstall } from "~/composables/useModInstall";
import { useNotifications } from "~/composables/useNotifications";
import { useProfileActivation } from "~/composables/useProfileActivation";
import { useProfiles } from "~/composables/useProfiles";
import { invoke } from "~/utils/tauri";
import { logger } from "~/utils/logger";

interface StartupLaunchOptions {
  profileName: string | null;
  launchGame: boolean;
}

const ERROR_TOAST_DURATION_MS = 12_000;

let startupArgsHandled = false;

export async function runStartupLaunchArgs() {
  if (startupArgsHandled) {
    return;
  }
  startupArgsHandled = true;

  const options = await invoke<StartupLaunchOptions>("get_startup_launch_options");
  if (!options.profileName && !options.launchGame) {
    return;
  }

  logger.info("Running startup launch options", options);

  const { pushNotification } = useNotifications();
  const { refreshProfiles, profiles, activeProfile } = useProfiles();
  const { refreshInstalled, resetSessionSync, syncSubscribedModsIfNeeded } =
    useModInstall();
  const { activateProfile, refreshActiveProfileMods } = useProfileActivation();

  try {
    await refreshProfiles();

    if (options.profileName) {
      const profile = profiles.value.find(
        (entry) => entry.name === options.profileName,
      );
      if (!profile) {
        pushNotification({
          title: "Profile not found",
          message: `No profile named "${options.profileName}".`,
          tone: "error",
          durationMs: ERROR_TOAST_DURATION_MS,
        });
        return;
      }

      if (!profile.selectable) {
        pushNotification({
          title: "Profile unavailable",
          message: "Sign in to use your account profile.",
          tone: "error",
          durationMs: ERROR_TOAST_DURATION_MS,
        });
        return;
      }

      if (!profile.isActive) {
        await activateProfile(profile);
      } else if (options.launchGame) {
        await refreshActiveProfileMods(profile);
      }
    } else if (options.launchGame) {
      await refreshInstalled({ force: true });
      if (activeProfile.value?.kind === "user") {
        resetSessionSync();
        await syncSubscribedModsIfNeeded();
      }
    }

    if (options.launchGame) {
      await launchGame();
    }
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    logger.error("Startup launch arguments failed", message);
    pushNotification({
      title: "Shortcut action failed",
      message,
      tone: "error",
      durationMs: ERROR_TOAST_DURATION_MS,
    });
  }
}
