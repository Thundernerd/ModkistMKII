const RATE_LIMIT_ERROR_REFS = new Set([11008, 11009]);

export function isRateLimitedMessage(message: string): boolean {
  const lower = message.toLowerCase();

  if (lower.includes("rate limit")) {
    return true;
  }

  if (lower.includes("too many requests")) {
    return true;
  }

  if (/\b429\b/.test(lower)) {
    return true;
  }

  const errorRefMatch = lower.match(/error_ref\s*(\d+)/);
  if (errorRefMatch) {
    const code = Number.parseInt(errorRefMatch[1] ?? "", 10);
    if (RATE_LIMIT_ERROR_REFS.has(code)) {
      return true;
    }
  }

  return false;
}

export function rateLimitUserMessage(): string {
  return "mod.io is limiting requests for this login. Wait about a minute, then try again.";
}

export function isRecoverableApiMessage(message: string): boolean {
  const trimmed = message.trim();
  if (!trimmed) {
    return false;
  }

  if (isRateLimitedMessage(trimmed)) {
    return true;
  }

  const lower = trimmed.toLowerCase();

  if (/error_ref\s*\d+/.test(lower)) {
    return true;
  }

  if (lower.includes("mod.io")) {
    return true;
  }

  if (lower.includes("not logged in") || lower.includes("sign in to mod.io")) {
    return true;
  }

  if (lower.includes("tauri ipc is not available")) {
    return true;
  }

  return false;
}

export function isRenderFailure(info: string | undefined): boolean {
  if (!info) {
    return false;
  }

  const lower = info.toLowerCase();
  return (
    lower.includes("render function") ||
    lower.includes("setup function") ||
    lower.includes("scheduler flush")
  );
}
