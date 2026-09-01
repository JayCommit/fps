import { useQuery } from "@tanstack/react-query";
import { Link } from "react-router-dom";
import { api } from "@fps/api-client";
import { StatusDot } from "../components/StatusDot";

export function DashboardPage() {
  const dash = useQuery({ queryKey: ["dashboard"], queryFn: api.dashboard, refetchInterval: 5_000 });

  if (dash.isError) {
    return <p role="alert">Could not load the dashboard. Check your session and try again.</p>;
  }
  if (!dash.data) {
    return <div className="h-40 animate-pulse rounded bg-[var(--bg-hover)]" aria-busy="true" />;
  }

  const d = dash.data;
  return (
    <div className="space-y-6">
      <header>
        <h1 className="text-2xl font-semibold">Operations</h1>
        <p className="text-[var(--text-muted)]">
          {d.product} {d.version} · invitation-only alpha
        </p>
      </header>
      <section className="grid gap-3 sm:grid-cols-2 lg:grid-cols-4">
        <Stat label="Nodes" value={d.nodes_total} />
        <Stat label="Online" value={d.nodes_online} tone="ok" />
        <Stat label="Degraded / offline" value={d.nodes_degraded + d.nodes_offline} tone={d.nodes_offline ? "danger" : "muted"} />
        <Stat label="Docker ready" value={d.docker_available} />
        {d.servers_total != null ? <Stat label="Servers" value={d.servers_total} /> : null}
        {d.servers_running != null ? <Stat label="Running" value={d.servers_running} tone="ok" /> : null}
      </section>
      <section>
        <h2 className="mb-3 text-sm font-medium uppercase tracking-wide text-[var(--text-faint)]">Alerts</h2>
        {d.alerts.length === 0 ? (
          <p className="text-[var(--text-muted)]">No active alerts.</p>
        ) : (
          <ul className="space-y-2">
            {d.alerts.map((a) => (
              <li
                key={a.title + a.detail}
                className="rounded-[var(--radius)] border border-[var(--border)] bg-[var(--bg-panel)] px-4 py-3"
              >
                <div className="flex items-center gap-2">
                  <StatusDot status={a.severity === "critical" ? "offline" : "degraded"} />
                  <strong>{a.title}</strong>
                </div>
                <p className="mt-1 text-sm text-[var(--text-muted)]">{a.detail}</p>
              </li>
            ))}
          </ul>
        )}
      </section>
      <p className="flex flex-wrap gap-4">
        <Link className="text-[var(--accent)] underline" to="/servers">
          Manage servers
        </Link>
        <Link className="text-[var(--accent)] underline" to="/nodes">
          Manage nodes
        </Link>
      </p>
    </div>
  );
}

function Stat({ label, value, tone = "muted" }: { label: string; value: number; tone?: string }) {
  const color =
    tone === "ok" ? "var(--ok)" : tone === "danger" ? "var(--danger)" : "var(--text)";
  return (
    <div className="rounded-[var(--radius)] border border-[var(--border)] bg-[var(--bg-panel)] px-4 py-3">
      <div className="text-xs uppercase tracking-wide text-[var(--text-faint)]">{label}</div>
      <div className="mt-1 font-mono text-2xl" style={{ color }}>
        {value}
      </div>
    </div>
  );
}
