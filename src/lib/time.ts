export function relativeTime(unixSecs: number): string {
  const diff = Math.max(0, Math.floor(Date.now() / 1000) - unixSecs);
  if (diff < 60) return "just now";
  if (diff < 3600) return `${Math.floor(diff / 60)}m ago`;
  if (diff < 86400) return `${Math.floor(diff / 3600)}h ago`;
  if (diff < 86400 * 30) return `${Math.floor(diff / 86400)}d ago`;
  return new Date(unixSecs * 1000).toLocaleDateString();
}

export function formatPlaytime(secs: number): string | null {
  if (secs < 60) return null;
  const hours = secs / 3600;
  if (hours < 1) return `${Math.floor(secs / 60)}m played`;
  return `${hours < 10 ? hours.toFixed(1) : Math.round(hours)}h played`;
}
