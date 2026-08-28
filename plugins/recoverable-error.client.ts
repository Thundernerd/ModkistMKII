import { useModioErrorFeedback } from "~/composables/useModioErrorFeedback";
import { useNotifications } from "~/composables/useNotifications";
import {
  isRecoverableApiMessage,
  isRenderFailure,
} from "~/utils/modioError";

function errorMessage(error: unknown): string {
  if (error instanceof Error) {
    return error.message;
  }

  if (typeof error === "object" && error !== null && "message" in error) {
    const message = (error as { message?: unknown }).message;
    if (typeof message === "string") {
      return message;
    }
  }

  return String(error);
}

export default defineNuxtPlugin((nuxtApp) => {
  const { notifyIfRateLimited } = useModioErrorFeedback();
  const { pushNotification } = useNotifications();

  function handleRecoverableError(error: unknown) {
    const message = errorMessage(error);
    if (!isRecoverableApiMessage(message)) {
      return;
    }

    if (!notifyIfRateLimited(message)) {
      pushNotification({
        title: "Request failed",
        message,
        tone: "error",
        durationMs: 10_000,
      });
    }

    clearError();
  }

  nuxtApp.hook("vue:error", (error, _instance, info) => {
    if (isRenderFailure(info)) {
      return;
    }

    handleRecoverableError(error);
  });

  nuxtApp.hook("app:error", (error) => {
    handleRecoverableError(error);
  });
});
