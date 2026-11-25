export function formatBytes(bytes: number | null | undefined): string {
  if (bytes == null || bytes < 0) return "—";
  if (bytes === 0) return "0 B";
  const units = ["B", "KB", "MB", "GB", "TB"];
  let value = bytes;
  let unit = 0;
  while (value >= 1024 && unit < units.length - 1) {
    value /= 1024;
    unit += 1;
  }
  const text =
    unit === 0 || value >= 100 ? Math.round(value).toString() : value.toFixed(1);
  return `${text} ${units[unit]}`;
}

export function tildify(path: string): string {
  return path.replace(/^\/Users\/[^/]+/, "~");
}
