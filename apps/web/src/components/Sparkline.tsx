export function Sparkline({
  values,
  label,
}: {
  values: number[];
  label: string;
}) {
  if (values.length < 2) {
    return <p className="text-sm text-[var(--text-muted)]">Not enough samples for {label} yet. Heartbeats fill this graph.</p>;
  }
  const min = Math.min(...values);
  const max = Math.max(...values);
  const span = max - min || 1;
  const w = 320;
  const h = 72;
  const d = values
    .map((v, i) => {
      const x = (i / (values.length - 1)) * w;
      const y = h - ((v - min) / span) * (h - 8) - 4;
      return `${i === 0 ? "M" : "L"}${x.toFixed(1)},${y.toFixed(1)}`;
    })
    .join(" ");
  return (
    <figure className="rounded-[var(--radius)] border border-[var(--border)] bg-[var(--bg-panel)] p-3">
      <figcaption className="mb-2 flex justify-between text-xs uppercase tracking-wide text-[var(--text-faint)]">
        <span>{label}</span>
        <span className="font-mono">
          {values[values.length - 1]?.toLocaleString()}
        </span>
      </figcaption>
      <svg viewBox={`0 0 ${w} ${h}`} className="h-20 w-full" role="img" aria-label={label}>
        <path d={d} fill="none" stroke="var(--accent)" strokeWidth="2" />
      </svg>
    </figure>
  );
}
