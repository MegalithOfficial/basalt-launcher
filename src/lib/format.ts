const KB = 1024;
const MB = 1024 ** 2;
const GB = 1024 ** 3;

function scaled(value: number, unit: string): string {
  return `${value < 10 ? value.toFixed(1) : Math.round(value)} ${unit}`;
}

export function formatBytes(bytes: number | null | undefined): string {
  if (bytes == null || bytes <= 0) return "0 B";
  if (bytes < KB) return `${Math.round(bytes)} B`;
  if (bytes < MB) return `${Math.round(bytes / KB)} KB`;
  if (bytes < GB) return scaled(bytes / MB, "MB");
  return scaled(bytes / GB, "GB");
}

export function formatMegabytes(mb: number | null | undefined): string {
  if (mb == null) return "unknown";
  return formatBytes(mb * MB);
}
