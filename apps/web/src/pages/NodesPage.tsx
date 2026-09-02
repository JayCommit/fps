import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Link } from "react-router-dom";
import { useState } from "react";
import { Cpu, HardDrive, Radio } from "lucide-react";
import { api, ApiError, getApiBase } from "@fps/api-client";
import { StatusDot } from "../components/StatusDot";
import {
  CopyButton,
  EmptyState,
  ErrorBanner,
  LoadingBlock,
  PageHeader,
  Panel,
  dangerBtn,
  primaryBtn,
} from "../components/PageStates";
import { formatBytes, formatRelative } from "../components/files";

export function NodesPage() {
  const qc = useQueryClient();
  const nodes = useQuery({ queryKey: ["nodes"], queryFn: api.nodes, refetchInterval: 5_000 });
  const settings = useQuery({ queryKey: ["settings"], queryFn: api.settings });
  const [token, setToken] = useState<string | null>(null);
  const [revokeError, setRevokeError] = useState<string | null>(null);
  const enroll = useMutation({
    mutationFn: () => api.createEnrollmentToken("web-ui"),
    onSuccess: (res) => {
      setToken(res.token);
      qc.invalidateQueries({ queryKey: ["nodes"] });
    },
  });
  const revoke = useMutation({
    mutationFn: api.revokeNode,
    onSuccess: () => {
      setRevokeError(null);
      qc.invalidateQueries({ queryKey: ["nodes"] });
      qc.invalidateQueries({ queryKey: ["dashboard"] });
    },
  });

  if (nodes.isError) {
    return <ErrorBanner error={nodes.error} fallback="Could not load nodes." />;
  }

  const apiUrl = (settings.data?.public_url || getApiBase() || "http://127.0.0.1:47890").replace(/\/$/, "");
  const insecure =
    settings.data?.allow_insecure_http ?? apiUrl.startsWith("http://");
  const enrollCmd = token
    ? `fps-node-agent enroll --url ${apiUrl} --token ${token} --data-dir /var/lib/fps/agent${
        insecure ? " --allow-insecure-http" : ""
      }`
    : "";

  return (
    <div className="space-y-6">
      <PageHeader
        title="Game nodes"
        description="Each host runs the node agent and Docker. Heartbeats drive live health, capacity, and job dispatch."
        actions={
          <button type="button" onClick={() => enroll.mutate()} className={primaryBtn}>
            <Radio size={16} /> {enroll.isPending ? "Issuing…" : "Issue enrollment token"}
          </button>
        }
      />

      {token ? (
        <Panel title="One-time enrollment token">
          <p className="text-sm text-[var(--text-muted)]">
            Shown once. Expires in 15 minutes and cannot be replayed. Run this on the game host (full VM, not LXC).
          </p>
          <code className="mt-3 block break-all rounded-[var(--radius)] bg-[var(--bg)] px-3 py-2 font-mono text-sm">
            {token}
          </code>
          <pre className="mt-3 overflow-auto rounded-[var(--radius)] bg-[var(--bg)] p-3 font-mono text-xs text-[var(--text-muted)]">
            {enrollCmd}
          </pre>
          <div className="mt-3 flex flex-wrap gap-2">
            <CopyButton text={token} label="Copy token" />
            <CopyButton text={enrollCmd} label="Copy enroll command" />
          </div>
        </Panel>
      ) : null}

      {revokeError ? <ErrorBanner error={new Error(revokeError)} fallback={revokeError} /> : null}

      {!nodes.data ? (
        <LoadingBlock />
      ) : nodes.data.length === 0 ? (
        <EmptyState>
          No nodes enrolled yet. Issue a token, then run the agent on a Ubuntu/Debian game host with Docker Engine.
        </EmptyState>
      ) : (
        <div className="grid gap-3 lg:grid-cols-2">
          {nodes.data.map((n) => {
            const mem = n.health.resources?.memory_bytes;
            const disk = n.health.resources?.disk_bytes;
            const diskFree = n.health.resources?.disk_available_bytes;
            const diskUsed =
              disk && diskFree != null ? Math.max(0, Math.min(100, Math.round(((disk - diskFree) / disk) * 100))) : null;
            return (
              <article
                key={n.id}
                className="rounded-[var(--radius)] border border-[var(--border)] bg-[var(--bg-panel)] p-4"
              >
                <div className="flex items-start justify-between gap-3">
                  <Link to={`/nodes/${n.id}`} className="min-w-0">
                    <div className="flex items-center gap-2">
                      <span
                        className="inline-flex h-10 w-10 items-center justify-center rounded-xl border border-[var(--border)] bg-[var(--bg)] font-mono text-xs text-[var(--accent)]"
                        aria-hidden
                      >
                        {n.health.status === "online" ? "ON" : n.health.status.slice(0, 3).toUpperCase()}
                      </span>
                      <div>
                        <h2 className="font-semibold hover:text-[var(--accent)]">{n.name}</h2>
                        <p className="font-mono text-xs text-[var(--text-muted)]">{n.hostname}</p>
                      </div>
                    </div>
                  </Link>
                  <span className="inline-flex items-center gap-1.5 rounded-full border border-[var(--border)] px-2 py-0.5 text-xs">
                    <StatusDot status={n.health.status} />
                    {n.health.status}
                  </span>
                </div>

                <dl className="mt-4 grid grid-cols-2 gap-3 text-sm">
                  <div>
                    <dt className="text-xs uppercase tracking-wide text-[var(--text-faint)]">Docker</dt>
                    <dd className="mt-0.5 font-mono text-xs">{n.health.docker}</dd>
                  </div>
                  <div>
                    <dt className="text-xs uppercase tracking-wide text-[var(--text-faint)]">Workloads</dt>
                    <dd className="mt-0.5 font-mono text-xs">{n.workload_count}</dd>
                  </div>
                  <div>
                    <dt className="text-xs uppercase tracking-wide text-[var(--text-faint)]">CPU</dt>
                    <dd className="mt-0.5 inline-flex items-center gap-1 font-mono text-xs">
                      <Cpu size={12} /> {n.health.resources?.cpu_cores ?? "—"} cores
                    </dd>
                  </div>
                  <div>
                    <dt className="text-xs uppercase tracking-wide text-[var(--text-faint)]">Memory</dt>
                    <dd className="mt-0.5 font-mono text-xs">{mem ? formatBytes(mem) : "—"}</dd>
                  </div>
                </dl>

                <div className="mt-4 space-y-2">
                  <div className="flex items-center justify-between text-xs text-[var(--text-faint)]">
                    <span className="inline-flex items-center gap-1">
                      <HardDrive size={12} /> Disk
                    </span>
                    <span className="font-mono">{diskUsed != null ? `${diskUsed}% used` : "unknown"}</span>
                  </div>
                  <div className="ui-bar">
                    <span style={{ width: `${diskUsed ?? 0}%` }} />
                  </div>
                </div>

                <div className="mt-4 flex items-center justify-between gap-2 text-xs text-[var(--text-muted)]">
                  <span>
                    Heartbeat {formatRelative(n.health.last_heartbeat_at)} · agent {n.health.agent_version ?? "—"}
                  </span>
                  <button
                    type="button"
                    className={dangerBtn}
                    disabled={revoke.isPending}
                    onClick={() => {
                      revoke.mutate(n.id, {
                        onError: (err) => {
                          setRevokeError(err instanceof ApiError ? err.message : "Could not revoke this node.");
                        },
                      });
                    }}
                  >
                    Revoke
                  </button>
                </div>
              </article>
            );
          })}
        </div>
      )}
    </div>
  );
}
