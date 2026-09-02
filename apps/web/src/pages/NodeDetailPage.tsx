import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Link, useNavigate, useParams } from "react-router-dom";
import { useEffect, useState, type FormEvent, type ReactNode } from "react";
import { Activity, Cpu, HardDrive, MemoryStick } from "lucide-react";
import { api, ApiError } from "@fps/api-client";
import { StatusDot } from "../components/StatusDot";
import {
  ErrorBanner,
  Field,
  Panel,
  dangerBtn,
  primaryBtn,
  secondaryBtn,
} from "../components/PageStates";
import { Sparkline } from "../components/Sparkline";
import { UsageBar } from "../components/UsageBar";
import { formatBytes, formatRelative, formatUptime, statusTone, usagePercent } from "../components/files";

export function NodeDetailPage() {
  const { id } = useParams();
  const navigate = useNavigate();
  const qc = useQueryClient();
  const [formError, setFormError] = useState<string | null>(null);
  const [name, setName] = useState("");
  const [labels, setLabels] = useState("");
  const [interval, setInterval] = useState("15");
  const [maintenance, setMaintenance] = useState(false);
  const [uninstallConfirm, setUninstallConfirm] = useState(false);

  const node = useQuery({
    queryKey: ["node", id],
    queryFn: () => api.node(id!),
    enabled: Boolean(id),
    refetchInterval: 5_000,
  });
  const samples = useQuery({
    queryKey: ["node-metrics", id],
    queryFn: () => api.nodeMetrics(id!),
    enabled: Boolean(id),
    refetchInterval: 15_000,
  });
  const servers = useQuery({
    queryKey: ["servers"],
    queryFn: api.servers,
    enabled: Boolean(id),
    refetchInterval: 5_000,
  });

  useEffect(() => {
    if (!node.data) return;
    setName(node.data.name);
    setLabels((node.data.labels ?? []).join(", "));
    setInterval(String(node.data.heartbeat_interval_seconds ?? 15));
    setMaintenance(Boolean(node.data.maintenance));
  }, [node.data]);

  const save = useMutation({
    mutationFn: () =>
      api.patchNode(id!, {
        name,
        labels: labels
          .split(",")
          .map((s) => s.trim())
          .filter(Boolean),
        maintenance,
        heartbeat_interval_seconds: Number(interval),
      }),
    onSuccess: () => {
      setFormError(null);
      qc.invalidateQueries({ queryKey: ["node", id] });
      qc.invalidateQueries({ queryKey: ["nodes"] });
      qc.invalidateQueries({ queryKey: ["dashboard"] });
    },
  });
  const prune = useMutation({
    mutationFn: () => api.pruneNodeDocker(id!),
    onSuccess: () => qc.invalidateQueries({ queryKey: ["node", id] }),
  });
  const uninstall = useMutation({
    mutationFn: () => api.uninstallNode(id!),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["node", id] });
      qc.invalidateQueries({ queryKey: ["nodes"] });
      qc.invalidateQueries({ queryKey: ["dashboard"] });
    },
  });
  const revoke = useMutation({
    mutationFn: () => api.revokeNode(id!),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["nodes"] });
      qc.invalidateQueries({ queryKey: ["dashboard"] });
      navigate("/nodes");
    },
  });

  if (node.isError) {
    return <ErrorBanner error={node.error} fallback="Node was not found or you do not have permission to view it." />;
  }
  if (!node.data) {
    return <div className="h-40 animate-pulse rounded bg-[var(--bg-hover)]" aria-busy="true" />;
  }

  const n = node.data;
  const r = n.health.resources ?? {};
  const memUsed = r.memory_used_bytes;
  const memTotal = r.memory_bytes;
  const diskTotal = r.disk_bytes;
  const diskFree = r.disk_available_bytes;
  const diskUsed = diskTotal != null && diskFree != null ? Math.max(0, diskTotal - diskFree) : undefined;
  const cpuPct = r.cpu_percent != null ? Math.round(r.cpu_percent) : null;
  const hostServers = (servers.data ?? []).filter((s) => s.node_id === n.id);
  const pending = save.isPending || prune.isPending || uninstall.isPending || revoke.isPending;

  function onSave(e: FormEvent) {
    e.preventDefault();
    setFormError(null);
    save.mutate(undefined, {
      onError: (err) => setFormError(err instanceof ApiError ? err.message : "Could not save host settings."),
    });
  }

  return (
    <div className="space-y-6">
      <Link to="/nodes" className="text-sm text-[var(--accent)]">
        ← Nodes
      </Link>

      <header className="flex flex-wrap items-start justify-between gap-4">
        <div>
          <div className="flex items-center gap-3">
            <StatusDot status={n.health.status} />
            <h1 className="text-2xl font-semibold tracking-tight">{n.name}</h1>
            <span className="rounded-full border border-[var(--border)] px-2 py-0.5 text-xs capitalize">
              {n.health.status}
            </span>
          </div>
          <p className="mt-1 font-mono text-sm text-[var(--text-muted)]">{n.hostname}</p>
          <p className="mt-1 text-sm text-[var(--text-muted)]">{n.health.message}</p>
        </div>
        <div className="flex flex-wrap gap-2">
          <button type="button" className={secondaryBtn} disabled={pending || n.revoked} onClick={() => prune.mutate()}>
            {prune.isPending ? "Pruning…" : "Docker prune"}
          </button>
          <button
            type="button"
            className={dangerBtn}
            disabled={pending || n.revoked}
            onClick={() => setUninstallConfirm(true)}
          >
            Uninstall host
          </button>
        </div>
      </header>

      {n.uninstall_requested && !n.uninstalled_at ? (
        <p className="rounded-[var(--radius)] border border-[var(--warn)]/40 bg-[var(--warn)]/10 px-3 py-2 text-sm">
          Uninstall requested. The agent will stop game containers, remove its identity, and disable itself on the next heartbeat.
        </p>
      ) : null}
      {n.uninstalled_at ? (
        <p className="rounded-[var(--radius)] border border-[var(--border)] px-3 py-2 text-sm text-[var(--text-muted)]">
          Agent uninstalled {formatRelative(n.uninstalled_at)}. Enrollment is revoked.
        </p>
      ) : null}
      {formError ? <ErrorBanner error={new Error(formError)} fallback={formError} /> : null}
      {save.isError ? <ErrorBanner error={save.error} fallback="Could not save host settings." /> : null}
      {prune.isError ? <ErrorBanner error={prune.error} fallback="Could not queue Docker prune." /> : null}
      {uninstall.isError ? <ErrorBanner error={uninstall.error} fallback="Could not request uninstall." /> : null}

      <section className="grid gap-3 sm:grid-cols-2 xl:grid-cols-4">
        <MetricCard
          icon={<Cpu size={16} />}
          label="CPU"
          value={cpuPct != null ? `${cpuPct}%` : "—"}
          hint={`${r.cpu_cores ?? "?"} cores · load ${r.load_one?.toFixed(2) ?? "—"}`}
        >
          <UsageBar label="Utilization" percent={cpuPct} />
        </MetricCard>
        <MetricCard
          icon={<MemoryStick size={16} />}
          label="Memory"
          value={memUsed != null ? formatBytes(memUsed) : "—"}
          hint={memTotal ? `of ${formatBytes(memTotal)}` : "host RAM"}
        >
          <UsageBar label="Used" percent={usagePercent(memUsed, memTotal)} detail={memTotal ? formatBytes(memTotal) : undefined} />
        </MetricCard>
        <MetricCard
          icon={<HardDrive size={16} />}
          label="Disk"
          value={diskUsed != null ? formatBytes(diskUsed) : "—"}
          hint={diskFree != null ? `${formatBytes(diskFree)} free` : "host volumes"}
        >
          <UsageBar label="Used" percent={usagePercent(diskUsed, diskTotal)} detail={diskFree != null ? `${formatBytes(diskFree)} free` : undefined} />
        </MetricCard>
        <MetricCard
          icon={<Activity size={16} />}
          label="Host"
          value={formatUptime(r.uptime_seconds)}
          hint={`Heartbeat ${formatRelative(n.health.last_heartbeat_at)}`}
        >
          <dl className="grid grid-cols-2 gap-2 text-xs text-[var(--text-muted)]">
            <div>
              <dt className="uppercase tracking-wide text-[var(--text-faint)]">Docker</dt>
              <dd className="font-mono">{n.health.docker}{n.docker_engine_version ? ` ${n.docker_engine_version}` : ""}</dd>
            </div>
            <div>
              <dt className="uppercase tracking-wide text-[var(--text-faint)]">Workloads</dt>
              <dd className="font-mono">{n.workload_count}</dd>
            </div>
          </dl>
        </MetricCard>
      </section>

      {samples.data && samples.data.length > 1 ? (
        <div className="grid gap-3 lg:grid-cols-2 xl:grid-cols-4">
          <Sparkline label="CPU %" values={samples.data.map((p) => p.cpu_percent ?? 0)} />
          <Sparkline label="Memory used" values={samples.data.map((p) => p.memory_bytes ?? 0)} />
          <Sparkline label="Disk free" values={samples.data.map((p) => p.disk_available_bytes ?? 0)} />
          <Sparkline label="Load (1m)" values={samples.data.map((p) => p.load_one ?? 0)} />
        </div>
      ) : (
        <p className="text-sm text-[var(--text-muted)]">Heartbeats will fill CPU, memory, disk, and load graphs.</p>
      )}

      <div className="grid gap-4 xl:grid-cols-[1.1fr_0.9fr]">
        <Panel title="Host settings">
          <form className="grid gap-3" onSubmit={onSave}>
            <Field
              id="node_name"
              label="Display name"
              value={name}
              onChange={(e) => setName(e.target.value)}
              disabled={n.revoked}
            />
            <Field
              id="node_labels"
              label="Labels"
              value={labels}
              onChange={(e) => setLabels(e.target.value)}
              hint="Comma-separated. Applied on the next heartbeat. Used for inventory, not scheduling yet."
              disabled={n.revoked}
            />
            <Field
              id="node_interval"
              label="Heartbeat interval (seconds)"
              type="number"
              min={5}
              max={300}
              value={interval}
              onChange={(e) => setInterval(e.target.value)}
              disabled={n.revoked}
            />
            <label className="flex items-center gap-2 text-sm">
              <input
                type="checkbox"
                checked={maintenance}
                onChange={(e) => setMaintenance(e.target.checked)}
                disabled={n.revoked}
              />
              Maintenance mode (do not schedule new servers)
            </label>
            <div>
              <button type="submit" className={primaryBtn} disabled={pending || n.revoked}>
                {save.isPending ? "Saving…" : "Save settings"}
              </button>
            </div>
          </form>
        </Panel>

        <Panel title="Identity">
          <dl className="grid gap-3 text-sm">
            <Row label="OS" value={`${n.operating_system ?? "—"} / ${n.architecture ?? "—"}`} />
            <Row label="Agent" value={n.health.agent_version ?? "—"} />
            <Row label="Protocol" value={String(n.health.protocol_version)} />
            <Row label="Enrolled" value={formatRelative(n.enrolled_at)} />
            <Row label="Node ID" value={n.id} mono />
          </dl>
        </Panel>
      </div>

      <Panel title="Servers on this host">
        {hostServers.length === 0 ? (
          <p className="text-sm text-[var(--text-muted)]">No game servers are assigned to this node yet.</p>
        ) : (
          <ul className="divide-y divide-[var(--border)]">
            {hostServers.map((s) => (
              <li key={s.id} className="flex items-center justify-between gap-3 py-2">
                <Link to={`/servers/${s.id}`} className="font-medium hover:text-[var(--accent)]">
                  {s.name}
                </Link>
                <span className="inline-flex items-center gap-2 text-sm">
                  <StatusDot status={statusTone(s.status)} />
                  {s.status}
                </span>
              </li>
            ))}
          </ul>
        )}
      </Panel>

      {uninstallConfirm ? (
        <Panel title="Uninstall this host">
          <p className="text-sm text-[var(--text-muted)]">
            The next heartbeat will stop FPS game containers, delete agent identity under /var/lib/fps/agent, and disable
            fps-node-agent. Binaries in /opt/fps are left on disk. This also drains the node.
          </p>
          <div className="mt-3 flex flex-wrap gap-2">
            <button
              type="button"
              className={dangerBtn}
              disabled={pending}
              onClick={() => uninstall.mutate(undefined, { onSuccess: () => setUninstallConfirm(false) })}
            >
              {uninstall.isPending ? "Requesting…" : "Confirm uninstall"}
            </button>
            <button type="button" className={secondaryBtn} onClick={() => setUninstallConfirm(false)}>
              Cancel
            </button>
          </div>
        </Panel>
      ) : null}

      <Panel
        title="Force revoke"
        actions={
          <button type="button" className={dangerBtn} disabled={pending || n.revoked} onClick={() => revoke.mutate()}>
            {revoke.isPending ? "Revoking…" : "Revoke enrollment now"}
          </button>
        }
      >
        <p className="text-sm text-[var(--text-muted)]">
          Immediate trust kill. The agent cannot heartbeat or receive jobs. Use uninstall first when the host is still
          online so it can clean itself up.
        </p>
      </Panel>
    </div>
  );
}

function MetricCard({
  icon,
  label,
  value,
  hint,
  children,
}: {
  icon: ReactNode;
  label: string;
  value: string;
  hint: string;
  children?: ReactNode;
}) {
  return (
    <article className="rounded-[var(--radius)] border border-[var(--border)] bg-[var(--bg-panel)] p-4">
      <div className="mb-3 flex items-center gap-2 text-xs uppercase tracking-wide text-[var(--text-faint)]">
        {icon}
        {label}
      </div>
      <p className="text-2xl font-semibold tracking-tight">{value}</p>
      <p className="mb-3 text-xs text-[var(--text-muted)]">{hint}</p>
      {children}
    </article>
  );
}

function Row({ label, value, mono }: { label: string; value: string; mono?: boolean }) {
  return (
    <div>
      <dt className="text-xs uppercase tracking-wide text-[var(--text-faint)]">{label}</dt>
      <dd className={mono ? "mt-0.5 break-all font-mono text-xs" : "mt-0.5"}>{value}</dd>
    </div>
  );
}
