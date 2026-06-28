export type NotificationTone = "error" | "info" | "success" | "warning";

export interface AppNotification {
  id: number;
  title: string;
  message: string;
  tone: NotificationTone;
}

const notifications = ref<AppNotification[]>([]);

let nextId = 1;

const DEFAULT_DURATION_MS = 8_000;

export function useNotifications() {
  function pushNotification(options: {
    title: string;
    message: string;
    tone?: NotificationTone;
    durationMs?: number;
  }) {
    const id = nextId++;
    const notification: AppNotification = {
      id,
      title: options.title,
      message: options.message,
      tone: options.tone ?? "info",
    };

    notifications.value = [...notifications.value, notification];

    const duration = options.durationMs ?? DEFAULT_DURATION_MS;
    if (duration > 0) {
      window.setTimeout(() => dismissNotification(id), duration);
    }

    return id;
  }

  function dismissNotification(id: number) {
    notifications.value = notifications.value.filter(
      (notification) => notification.id !== id,
    );
  }

  return {
    notifications,
    pushNotification,
    dismissNotification,
  };
}
