function diffParts(iso: string) {
  const date = new Date(iso);
  if (Number.isNaN(date.getTime())) {
    return null;
  }

  const now = Date.now();
  const diffMs = Math.max(0, now - date.getTime());
  const diffMin = Math.floor(diffMs / 60_000);
  const diffHour = Math.floor(diffMin / 60);
  const diffDay = Math.floor(diffHour / 24);
  const diffMonth = Math.floor(diffDay / 30);
  const remainDay = diffDay % 30;

  return { diffMin, diffHour, diffDay, diffMonth, remainDay };
}

/** Compact relative time, e.g. "1d", "5mo 9d". */
export function formatRelativeShort(iso: string) {
  const parts = diffParts(iso);
  if (!parts) return "";

  const { diffMin, diffHour, diffDay, diffMonth, remainDay } = parts;

  if (diffDay < 1) {
    if (diffHour < 1) {
      return diffMin <= 1 ? "just now" : `${diffMin}m`;
    }
    return `${diffHour}h`;
  }

  if (diffDay < 30) {
    return `${diffDay}d`;
  }

  if (diffMonth < 12) {
    return remainDay > 0 ? `${diffMonth}mo ${remainDay}d` : `${diffMonth}mo`;
  }

  const diffYear = Math.floor(diffMonth / 12);
  const remainMonth = diffMonth % 12;
  return remainMonth > 0 ? `${diffYear}y ${remainMonth}mo` : `${diffYear}y`;
}

/** Relative time with trailing "ago", e.g. "5mo 9d ago". */
export function formatRelativeAgo(iso: string) {
  const short = formatRelativeShort(iso);
  if (!short) return "";
  return short === "just now" ? short : `${short} ago`;
}

export function formatFileSize(bytes: number | null | undefined) {
  if (!bytes || bytes <= 0) return "";
  const units = ["B", "KB", "MB", "GB"];
  let value = bytes;
  let unitIndex = 0;

  while (value >= 1024 && unitIndex < units.length - 1) {
    value /= 1024;
    unitIndex += 1;
  }

  return `${value.toFixed(unitIndex === 0 ? 0 : 2)} ${units[unitIndex]}`;
}
