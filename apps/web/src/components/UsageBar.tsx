export function UsageBar({
  label,
  percent,
  detail,
  warnAt = 85,
}: {
  label: string;
  percent: number | null;
  detail?: string;
  warnAt?: number;
}) {
  const tone =
    percent == null ? "var(--text-faint)" : percent >= warnAt ? "var(--danger)" : percent >= warnAt - 15 ? "var(--warn)" : "var(--accent)";
  return (
    <div>
      <div className="mb-1 flex items-center justify-between text-xs">
        <span className="uppercase tracking-wide text-[var(--text-faint)]">{label}</span>
        <span className="font-mono text-[var(--text-muted)]">
          {percent != null ? `${percent}%` : "—"}
          {detail ? ` · ${detail}` : ""}
        </span>
      </div>
      <div className="ui-bar">
        <span style={{ width: `${percent ?? 0}%`, background: tone }} />
      </div>
    </div>
  );
}
