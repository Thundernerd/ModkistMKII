import { useModioRateLimit } from "~/composables/useModioRateLimit";
import { useNotifications } from "~/composables/useNotifications";
import { isRateLimitedMessage, rateLimitUserMessage } from "~/utils/modioError";

const RATE_LIMIT_TOAST_DURATION_MS = 10_000;

export function useModioErrorFeedback() {
  const { pushNotification } = useNotifications();
  const { showRateLimitBanner } = useModioRateLimit();

  function notifyRateLimit() {
    showRateLimitBanner();
    pushNotification({
      title: "mod.io rate limit",
      message: rateLimitUserMessage(),
      tone: "warning",
      durationMs: RATE_LIMIT_TOAST_DURATION_MS,
    });
  }

  function notifyIfRateLimited(message: string) {
    if (!isRateLimitedMessage(message)) {
      return false;
    }

    notifyRateLimit();
    return true;
  }

  async function catchModAction(action: () => Promise<unknown>) {
    try {
      await action();
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      notifyIfRateLimited(message);
    }
  }

  return {
    notifyRateLimit,
    notifyIfRateLimited,
    catchModAction,
  };
}
