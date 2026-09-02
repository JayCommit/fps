import { useQuery } from "@tanstack/react-query";
import { Link } from "react-router-dom";
import type { ReactNode } from "react";
import { Activity, Box, Cpu, Server, ShieldAlert } from "lucide-react";
import { api } from "@fps/api-client";
import { StatusDot } from "../components/StatusDot";
import { GameIcon } from "../components/GameIcon";
import { EmptyState, ErrorBanner, LoadingBlock, PageHeader, primaryBtn, secondaryBtn } from "../components/PageStates";
import { formatRelative, statusTone } from "../components/files";

export function DashboardPage() {
  const dash = useQuery({ queryKey: ["dashboard"], queryFn: api.dashboard, refetchInterval: 5_000 });
  const nodes = useQuery({ queryKey: ["nodes"], queryFn: api.nodes, refetchInterval: 5_000 });
  const servers = useQuery({ queryKey: ["servers"], queryFn: api.servers, refetchInterval: 5_000 });
  const templates = useQuery({ queryKey: ["templates"], queryFn: api.templates });

  if (dash.isError) {
    return <ErrorBanner error={dash.error} fallback="Could not load the dashboard. Check your session and try again." />;
  }
  if (!dash.data) {
    return <LoadingBlock />;
  }

  const d = dash.data;
  const onlinePct = d.nodes_total ? Math.round((d.nodes_online / d.nodes_total) * 100) : 0;
  const runningPct = d.servers_total ? Math.round(((d.servers_running ?? 0) / d.servers_total) * 100) : 0;
  const tplById = new Map((templates.data ?? []).map((t) => [t.id, t]));
  const recent = (servers.data ?? []).slice(0, 6);

  return (
    <div className="space-y-6">
      <PageHeader
        title="Operations"
        description={`${d.product} ${d.version} · live health from enrolled agents`}
        actions={
          <>
            <Link to="/servers/new" className={primaryBtn}>
              Deploy server
            </Link>
            <Link to="/nodes" className={secondaryBtn}>
              Enroll node
            </Link>
          </>
        }
      />

      <section className="grid gap-3 sm:grid-cols-2 xl:grid-cols-4">
        <Stat
          icon={<Server size={16} />}
          label="Nodes online"
          value={`${d.nodes_online}/${d.nodes_total}`}
          hint={`${d.docker_available} with Docker ready`}
          bar={onlinePct}
          tone={d.nodes_offline ? "danger" : "ok"}
        />
        <Stat
          icon={<Box size={16} />}
          label="Servers running"
          value={`${d.servers_running ?? 0}/${d.servers_total ?? 0}`}
          hint="Scheduled onto enrolled hosts"
          bar={runningPct}
          tone="ok"
        />
        <Stat
          icon={<Activity size={16} />}
          label="Degraded / offline"
          value={d.nodes_degraded + d.nodes_offline}
          hint={d.nodes_offline ? "A host missed heartbeats" : "Cluster looks healthy"}
          tone={d.nodes_offline ? "danger" : "muted"}
        />
        <Stat
          icon={<Cpu size={16} />}
          label="Templates"
          value={templates.data?.length ?? 0}
          hint="Native catalogue + imports"
        />
      </section>

      <div className="grid gap-4 xl:grid-cols-[1.2fr_0.8fr]">
        <section className="rounded-[var(--radius)] border border-[var(--border)] bg-[var(--bg-panel)] p-4">
          <div className="mb-3 flex items-center justify-between">
            <h2 className="text-sm font-medium uppercase tracking-wide text-[var(--text-faint)]">Game servers</h2>
            <Link className="text-sm text-[var(--accent)]" to="/servers">
              View all
            </Link>
          </div>
          {servers.isError ? (
            <ErrorBanner error={servers.error} fallback="Could not load servers." />
          ) : !servers.data ? (
            <LoadingBlock />
          ) : recent.length === 0 ? (
            <EmptyState>
              Nothing deployed yet.{" "}
              <Link className="text-[var(--accent)]" to="/servers/new">
                Deploy from a template
              </Link>
              .
            </EmptyState>
          ) : (
            <ul className="divide-y divide-[var(--border)]">
              {recent.map((s) => {
                const tpl = tplById.get(s.template_id);
                return (
                  <li key={s.id} className="flex items-center gap-3 py-3">
                    <GameIcon slug={tpl?.slug} name={tpl?.name ?? s.name} game={tpl?.game} size="sm" />
                    <div className="min-w-0 flex-1">
                      <Link className="font-medium hover:text-[var(--accent)]" to={`/servers/${s.id}`}>
                        {s.name}
                      </Link>
                      <div className="truncate text-xs text-[var(--text-muted)]">
                        {tpl?.name ?? "template"} · {s.memory_mb} MiB · {formatRelative(s.created_at)}
                      </div>
                    </div>
                    <span className="inline-flex items-center gap-2 text-sm">
                      <StatusDot status={statusTone(s.status)} />
                      {s.status}
                    </span>
                  </li>
                );
              })}
            </ul>
          )}
        </section>

        <section className="space-y-4">
          <div className="rounded-[var(--radius)] border border-[var(--border)] bg-[var(--bg-panel)] p-4">
            <h2 className="mb-3 text-sm font-medium uppercase tracking-wide text-[var(--text-faint)]">Hosts</h2>
            {!nodes.data || nodes.data.length === 0 ? (
              <p className="text-sm text-[var(--text-muted)]">
                No nodes enrolled.{" "}
                <Link className="text-[var(--accent)]" to="/nodes">
                  Issue a token
                </Link>
                .
              </p>
            ) : (
              <ul className="space-y-2">
                {nodes.data.slice(0, 5).map((n) => (
                  <li key={n.id}>
                    <Link
                      to={`/nodes/${n.id}`}
                      className="flex items-center justify-between rounded-[var(--radius)] border border-transparent px-2 py-2 hover:border-[var(--border)] hover:bg-[var(--bg-hover)]"
                    >
                      <span className="inline-flex items-center gap-2">
                        <StatusDot status={n.health.status} />
                        <span className="font-medium">{n.name}</span>
                      </span>
                      <span className="font-mono text-xs text-[var(--text-faint)]">
                        {n.workload_count} srv
                        {n.health.resources?.cpu_percent != null
                          ? ` · ${Math.round(n.health.resources.cpu_percent)}% CPU`
                          : ""}
                      </span>
                    </Link>
                  </li>
                ))}
              </ul>
            )}
          </div>

          <div className="rounded-[var(--radius)] border border-[var(--border)] bg-[var(--bg-panel)] p-4">
            <h2 className="mb-3 flex items-center gap-2 text-sm font-medium uppercase tracking-wide text-[var(--text-faint)]">
              <ShieldAlert size={14} /> Alerts
            </h2>
            {d.alerts.length === 0 ? (
              <p className="text-sm text-[var(--text-muted)]">No active alerts. Cluster is quiet.</p>
            ) : (
              <ul className="space-y-2">
                {d.alerts.map((a) => (
                  <li
                    key={a.title + a.detail}
                    className="rounded-[var(--radius)] border border-[var(--border)] bg-[var(--bg)] px-3 py-2"
                  >
                    <div className="flex items-center gap-2">
                      <StatusDot status={a.severity === "critical" ? "offline" : "degraded"} />
                      <strong className="text-sm">{a.title}</strong>
                    </div>
                    <p className="mt-1 text-xs text-[var(--text-muted)]">{a.detail}</p>
                  </li>
                ))}
              </ul>
            )}
          </div>
        </section>
      </div>
    </div>
  );
}

function Stat({
  label,
  value,
  hint,
  tone = "muted",
  bar,
  icon,
}: {
  label: string;
  value: number | string;
  hint?: string;
  tone?: string;
  bar?: number;
  icon: ReactNode;
}) {
  const color = tone === "ok" ? "var(--ok)" : tone === "danger" ? "var(--danger)" : "var(--text)";
  return (
    <div className="rounded-[var(--radius)] border border-[var(--border)] bg-[var(--bg-panel)] px-4 py-4">
      <div className="flex items-center justify-between text-xs uppercase tracking-wide text-[var(--text-faint)]">
        <span>{label}</span>
        <span className="text-[var(--text-muted)]">{icon}</span>
      </div>
      <div className="mt-2 font-mono text-3xl tracking-tight" style={{ color }}>
        {value}
      </div>
      {hint ? <p className="mt-1 text-xs text-[var(--text-muted)]">{hint}</p> : null}
      {bar != null ? (
        <div className="ui-bar mt-3">
          <span style={{ width: `${Math.max(0, Math.min(100, bar))}%` }} />
        </div>
      ) : null}
    </div>
  );
}
