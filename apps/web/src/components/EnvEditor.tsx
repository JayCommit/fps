import { Plus, Trash2 } from "lucide-react";
import { useMemo } from "react";
import { secondaryBtn } from "./PageStates";

const SECRET_HINT = /pass|token|secret|key|license|rcon/i;

export type EnvMap = Record<string, string>;

export function envToRows(env?: EnvMap | null): { key: string; value: string }[] {
  const entries = Object.entries(env ?? {});
  if (entries.length === 0) return [{ key: "", value: "" }];
  return entries.map(([key, value]) => ({ key, value }));
}

export function rowsToEnv(rows: { key: string; value: string }[]): EnvMap {
  const out: EnvMap = {};
  for (const row of rows) {
    const key = row.key.trim();
    if (!key) continue;
    out[key] = row.value;
  }
  return out;
}

export function EnvEditor({
  id,
  label = "Environment",
  hint,
  rows,
  onChange,
}: {
  id?: string;
  label?: string;
  hint?: string;
  rows: { key: string; value: string }[];
  onChange: (rows: { key: string; value: string }[]) => void;
}) {
  const filled = useMemo(() => rows.filter((r) => r.key.trim()).length, [rows]);

  function update(index: number, patch: Partial<{ key: string; value: string }>) {
    onChange(rows.map((row, i) => (i === index ? { ...row, ...patch } : row)));
  }

  return (
    <div>
      <div className="mb-2 flex items-end justify-between gap-3">
        <label className="block text-sm text-[var(--text-muted)]" htmlFor={id}>
          {label}
          <span className="ml-2 font-mono text-xs text-[var(--text-faint)]">{filled} vars</span>
        </label>
        <button
          type="button"
          className={secondaryBtn}
          onClick={() => onChange([...rows, { key: "", value: "" }])}
        >
          <span className="inline-flex items-center gap-1">
            <Plus size={14} /> Add variable
          </span>
        </button>
      </div>
      <div className="overflow-hidden rounded-[var(--radius)] border border-[var(--border)]">
        <div className="grid grid-cols-[minmax(8rem,0.4fr)_1fr_auto] gap-px bg-[var(--border)] text-xs uppercase tracking-wide text-[var(--text-faint)]">
          <div className="bg-[var(--bg-raised)] px-3 py-2">Key</div>
          <div className="bg-[var(--bg-raised)] px-3 py-2">Value</div>
          <div className="bg-[var(--bg-raised)] px-3 py-2" />
        </div>
        {rows.map((row, index) => (
          <div
            key={index}
            className="grid grid-cols-[minmax(8rem,0.4fr)_1fr_auto] gap-px bg-[var(--border)]"
          >
            <input
              id={index === 0 ? id : undefined}
              name={index === 0 ? id : undefined}
              value={row.key}
              onChange={(e) => update(index, { key: e.target.value })}
              placeholder="EULA"
              autoComplete="off"
              spellCheck={false}
              className="bg-[var(--bg)] px-3 py-2 font-mono text-sm outline-none focus:bg-[var(--bg-hover)]"
            />
            <input
              value={row.value}
              onChange={(e) => update(index, { value: e.target.value })}
              placeholder="TRUE"
              autoComplete="off"
              spellCheck={false}
              type={SECRET_HINT.test(row.key) ? "password" : "text"}
              className="bg-[var(--bg)] px-3 py-2 font-mono text-sm outline-none focus:bg-[var(--bg-hover)]"
            />
            <button
              type="button"
              className="bg-[var(--bg)] px-3 text-[var(--text-faint)] hover:text-[var(--danger)]"
              aria-label={`Remove ${row.key || "variable"}`}
              onClick={() => {
                const next = rows.filter((_, i) => i !== index);
                onChange(next.length ? next : [{ key: "", value: "" }]);
              }}
            >
              <Trash2 size={14} />
            </button>
          </div>
        ))}
      </div>
      {hint ? <p className="mt-1 text-xs text-[var(--text-faint)]">{hint}</p> : null}
    </div>
  );
}
