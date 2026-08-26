// Formatting helpers. Every number the operator reads goes through one of
// these, so units and precision are consistent across every page.

const BYTE_UNITS = ['B', 'KB', 'MB', 'GB', 'TB', 'PB', 'EB'];

/** 1_234_567 → "1.2 MB". Decimal units, because disks are sold that way. */
export function bytes(n: number, digits = 1): string {
  if (!Number.isFinite(n)) return '—';
  if (n === 0) return '0 B';
  const i = Math.min(Math.floor(Math.log10(Math.abs(n)) / 3), BYTE_UNITS.length - 1);
  const v = n / 1000 ** i;
  return `${v.toFixed(i === 0 ? 0 : v >= 100 ? 0 : digits)} ${BYTE_UNITS[i]}`;
}

const BIBYTE_UNITS = ['B', 'KiB', 'MiB', 'GiB', 'TiB', 'PiB'];

/** Powers of two, for the values that actually are: block size, fragment size,
 *  the mapped block map. Disks are sold in decimal; block sizes are not, and
 *  showing a 128 MiB block as "134 MB" reads as a bug. */
export function bibytes(n: number, digits = 0): string {
  if (!Number.isFinite(n)) return '—';
  if (n === 0) return '0 B';
  const i = Math.min(Math.floor(Math.log2(Math.abs(n)) / 10), BIBYTE_UNITS.length - 1);
  const v = n / 1024 ** i;
  return `${v.toFixed(i === 0 || Number.isInteger(v) ? 0 : digits)} ${BIBYTE_UNITS[i]}`;
}

/** Bytes per second, same scale. */
export function rate(n: number): string {
  return n > 0 ? `${bytes(n)}/s` : '—';
}

/** 4_201_882 → "4.2M". For counts, where the unit is implied. */
export function count(n: number): string {
  if (!Number.isFinite(n)) return '—';
  if (Math.abs(n) < 1000) return String(Math.round(n));
  const units = ['', 'k', 'M', 'B', 'T'];
  const i = Math.min(Math.floor(Math.log10(Math.abs(n)) / 3), units.length - 1);
  const v = n / 1000 ** i;
  return `${v >= 100 ? v.toFixed(0) : v.toFixed(1)}${units[i]}`;
}

export function pct(part: number, whole: number, digits = 0): string {
  if (!whole) return '0%';
  return `${((part / whole) * 100).toFixed(digits)}%`;
}

export function pctValue(part: number, whole: number): number {
  return whole ? (part / whole) * 100 : 0;
}

/** Seconds → "4m 12s". Compact, and never more than two units. */
export function duration(seconds: number): string {
  if (!Number.isFinite(seconds) || seconds < 0) return '—';
  if (seconds < 1) return `${Math.round(seconds * 1000)}ms`;
  if (seconds < 60) return `${seconds < 10 ? seconds.toFixed(1) : Math.round(seconds)}s`;
  const m = Math.floor(seconds / 60);
  const s = Math.round(seconds % 60);
  if (m < 60) return s ? `${m}m ${s}s` : `${m}m`;
  const h = Math.floor(m / 60);
  return `${h}h ${m % 60}m`;
}

/** Milliseconds, for latencies. Sub-millisecond values keep a decimal. */
export function ms(v: number): string {
  if (!Number.isFinite(v)) return '—';
  if (v === 0) return '0 ms';
  return v < 10 ? `${v.toFixed(1)} ms` : `${Math.round(v)} ms`;
}

/** An epoch-ms timestamp → "12m ago", "3h ago", "26 Aug". */
export function ago(epochMs: number, now = Date.now()): string {
  const s = (now - epochMs) / 1000;
  if (s < 45) return 'just now';
  if (s < 3600) return `${Math.round(s / 60)}m ago`;
  if (s < 86400) return `${Math.round(s / 3600)}h ago`;
  return new Date(epochMs).toLocaleDateString(undefined, { day: 'numeric', month: 'short' });
}

export function clock(epochMs: number): string {
  return new Date(epochMs).toLocaleTimeString(undefined, {
    hour: '2-digit',
    minute: '2-digit',
    second: '2-digit',
  });
}

/** "/warehouse/events/dt=2026-08-03" → ["warehouse", "events", "dt=2026-08-03"] */
export function segments(path: string): string[] {
  return path.split('/').filter(Boolean);
}

export function parentOf(path: string): string {
  const parts = segments(path);
  parts.pop();
  return '/' + parts.join('/');
}

export function joinPath(base: string, name: string): string {
  return base === '/' ? `/${name}` : `${base}/${name}`;
}
