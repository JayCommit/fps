export type FileEntry = {
  name: string;
  path?: string;
  size?: number;
  is_dir?: boolean;
  modified_at?: string;
};

function asEntry(item: unknown): FileEntry {
  if (typeof item === "string") {
    return { name: item, path: item };
  }
  if (item && typeof item === "object") {
    const o = item as Record<string, unknown>;
    const name = String(o.name ?? o.path ?? o.filename ?? o.key ?? "file");
    return {
      name,
      path: o.path != null ? String(o.path) : undefined,
      size: typeof o.size === "number" ? o.size : typeof o.size_bytes === "number" ? o.size_bytes : undefined,
      is_dir: Boolean(o.is_dir ?? o.directory ?? o.dir),
      modified_at:
        o.modified_at != null
          ? String(o.modified_at)
          : o.mtime != null
            ? String(o.mtime)
            : undefined,
    };
  }
  return { name: String(item) };
}

/** Normalize GET /v1/servers/:id/files payloads (array, `{ files }`, or a name map). */
export function normalizeFiles(data: unknown): FileEntry[] {
  if (!data) return [];
  if (Array.isArray(data)) return data.map(asEntry);
  if (typeof data === "object") {
    const bag = data as { files?: unknown; entries?: unknown };
    const nested = bag.files ?? bag.entries;
    if (Array.isArray(nested)) return nested.map(asEntry);
    if (nested && typeof nested === "object" && !Array.isArray(nested)) {
      return Object.entries(nested as Record<string, unknown>).map(([key, value]) => {
        if (value && typeof value === "object") {
          return asEntry({ name: key, ...(value as object) });
        }
        return { name: key, path: key };
      });
    }
    return Object.keys(data as object)
      .filter((k) => k !== "files" && k !== "entries")
      .map((k) => ({ name: k, path: k }));
  }
  return [];
}

export function formatBytes(n?: number | null) {
  if (n == null || Number.isNaN(n)) return "—";
  if (n < 1024) return `${n} B`;
  const kib = n / 1024;
  if (kib < 1024) return `${kib.toFixed(1)} KiB`;
  const mib = kib / 1024;
  if (mib < 1024) return `${mib.toFixed(1)} MiB`;
  return `${(mib / 1024).toFixed(1)} GiB`;
}

export function formatWhen(value?: string | null) {
  if (!value) return "—";
  const d = new Date(value);
  return Number.isNaN(d.getTime()) ? value : d.toLocaleString();
}

export function statusTone(status: string): string {
  const s = status.toLowerCase();
  if (["running", "online", "ok", "succeeded", "active", "available"].includes(s)) return "online";
  if (["failed", "offline", "critical", "error", "disabled"].includes(s)) return "offline";
  if (["degraded", "warning", "installing", "pending", "stopped", "queued"].includes(s)) return "degraded";
  return s;
}
