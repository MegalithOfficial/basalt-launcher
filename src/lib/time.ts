export function relativeTime(unixSecs: number): string {
  const diff = Math.max(0, Math.floor(Date.now() / 1000) - unixSecs);
  if (diff < 60) return "just now";
  if (diff < 3600) return `${Math.floor(diff / 60)}m ago`;
  if (diff < 86400) return `${Math.floor(diff / 3600)}h ago`;
  if (diff < 86400 * 30) return `${Math.floor(diff / 86400)}d ago`;
  return new Date(unixSecs * 1000).toLocaleDateString();
}

export function durationParts(secs: number): Array<{ value: string; unit: string }> {
  if (secs < 60) return [{ value: String(Math.max(0, secs)), unit: "s" }];
  const hours = Math.floor(secs / 3600);
  const minutes = Math.floor((secs % 3600) / 60);
  if (hours === 0) return [{ value: String(minutes), unit: "m" }];
  const parts = [{ value: String(hours), unit: "h" }];
  if (minutes > 0) parts.push({ value: String(minutes), unit: "m" });
  return parts;
}

export function formatDuration(secs: number): string {
  if (secs <= 0) return "0m";
  return durationParts(secs)
    .map((part) => `${part.value}${part.unit}`)
    .join(" ");
}

export function formatDateTime(when: number | string | Date): string {
  return new Date(when).toLocaleString(undefined, {
    dateStyle: "medium",
    timeStyle: "short",
  });
}

export function formatPlaytime(secs: number): string | null {
  if (secs < 60) return null;
  const hours = secs / 3600;
  if (hours < 1) return `${Math.floor(secs / 60)}m played`;
  return `${hours < 10 ? hours.toFixed(1) : Math.round(hours)}h played`;
}
