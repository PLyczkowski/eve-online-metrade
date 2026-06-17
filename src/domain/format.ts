export function formatIsk(value: number | null): string {
  if (value === null || Number.isNaN(value)) return "";
  return new Intl.NumberFormat("en-US", {
    maximumFractionDigits: value < 100 ? 2 : 0
  }).format(value);
}

export function formatPercent(value: number | null): string {
  if (value === null || Number.isNaN(value)) return "";
  return `${(value * 100).toFixed(2)}%`;
}

export function minutesAgo(value: string | null): number | null {
  if (!value) return null;
  const time = new Date(value.replace(" ", "T")).getTime();
  if (Number.isNaN(time)) return null;
  return Math.max(0, Math.round((Date.now() - time) / 60000));
}
