export function StatusDot({ status }: { status: string }) {
  const color =
    status === "online" || status === "ok"
      ? "var(--ok)"
      : status === "degraded" || status === "warning" || status === "maintenance"
        ? "var(--warn)"
        : status === "offline" || status === "critical"
          ? "var(--danger)"
          : "var(--text-faint)";
  return (
    <span
      className="inline-block h-2.5 w-2.5 rounded-full"
      style={{ background: color }}
      aria-hidden
    />
  );
}
