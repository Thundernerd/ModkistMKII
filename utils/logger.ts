const PREFIX = "[Modkist]";

function emit(
  level: "debug" | "info" | "warn" | "error",
  message: string,
  detail?: unknown,
) {
  const line = detail === undefined ? message : `${message}`;
  const args = detail === undefined ? [PREFIX, line] : [PREFIX, line, detail];
  console[level](...args);
}

export const logger = {
  debug(message: string, detail?: unknown) {
    if (import.meta.dev) {
      emit("debug", message, detail);
    }
  },
  info(message: string, detail?: unknown) {
    emit("info", message, detail);
  },
  warn(message: string, detail?: unknown) {
    emit("warn", message, detail);
  },
  error(message: string, detail?: unknown) {
    emit("error", message, detail);
  },
};
