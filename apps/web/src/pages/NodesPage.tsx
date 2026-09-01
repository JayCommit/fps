import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Link } from "react-router-dom";
import { useState } from "react";
import { api, ApiError } from "@fps/api-client";
import { StatusDot } from "../components/StatusDot";
import { dangerBtn } from "../components/PageStates";

export function NodesPage() {
  const qc = useQueryClient();
  const nodes = useQuery({ queryKey: ["nodes"], queryFn: api.nodes, refetchInterval: 5_000 });
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
    return (
      <p role="alert">
        {nodes.error instanceof ApiError ? nodes.error.message : "Could not load nodes."}
      </p>
    );
  }

  return (
    <div className="space-y-6">
      <header className="flex flex-wrap items-end justify-between gap-4">
        <div>
          <h1 className="text-2xl font-semibold">Game nodes</h1>
          <p className="text-[var(--text-muted)]">
            Enroll an agent with a one-time token. Heartbeats drive live health.
          </p>
        </div>
        <button
          type="button"
          onClick={() => enroll.mutate()}
          className="rounded-[var(--radius)] bg-[var(--accent)] px-4 py-2 font-medium text-[#06221c]"
        >
          {enroll.isPending ? "Issuing…" : "Issue enrollment token"}
        </button>
      </header>

      {token ? (
        <div className="rounded-[var(--radius)] border border-[var(--accent)]/40 bg-[var(--accent-dim)] p-4">
          <p className="text-sm">One-time token (shown once). It expires in 15 minutes and cannot be replayed.</p>
          <code className="mt-2 block break-all font-mono text-sm">{token}</code>
          <pre className="mt-3 overflow-auto text-xs text-[var(--text-muted)]">{`fps-node-agent enroll --url http://127.0.0.1:47890 --token ${token}`}</pre>
        </div>
      ) : null}

      {revokeError ? (
        <p role="alert" className="text-sm text-[var(--danger)]">
          {revokeError}
        </p>
      ) : null}

      {!nodes.data ? (
        <div className="h-40 animate-pulse rounded bg-[var(--bg-hover)]" aria-busy="true" />
      ) : nodes.data.length === 0 ? (
        <div className="rounded-[var(--radius)] border border-dashed border-[var(--border-strong)] p-8 text-[var(--text-muted)]">
          No nodes enrolled yet. Issue a token and run the agent on the game-node VM (or locally for development).
        </div>
      ) : (
        <div className="overflow-x-auto rounded-[var(--radius)] border border-[var(--border)]">
          <table className="w-full text-left text-sm">
            <thead className="bg-[var(--bg-raised)] text-xs uppercase tracking-wide text-[var(--text-faint)]">
              <tr>
                <th className="px-4 py-2">Node</th>
                <th className="px-4 py-2">Status</th>
                <th className="px-4 py-2">Docker</th>
                <th className="px-4 py-2">Heartbeat</th>
                <th className="px-4 py-2">Agent</th>
                <th className="px-4 py-2" />
              </tr>
            </thead>
            <tbody>
              {nodes.data.map((n) => (
                <tr key={n.id} className="border-t border-[var(--border)]">
                  <td className="px-4 py-3">
                    <Link className="font-medium text-[var(--accent)]" to={`/nodes/${n.id}`}>
                      {n.name}
                    </Link>
                    <div className="font-mono text-xs text-[var(--text-muted)]">{n.hostname}</div>
                  </td>
                  <td className="px-4 py-3">
                    <span className="inline-flex items-center gap-2">
                      <StatusDot status={n.health.status} />
                      {n.health.status}
                    </span>
                  </td>
                  <td className="px-4 py-3 font-mono text-xs">{n.health.docker}</td>
                  <td className="px-4 py-3 font-mono text-xs">
                    {n.health.last_heartbeat_at
                      ? new Date(n.health.last_heartbeat_at).toLocaleString()
                      : "never"}
                  </td>
                  <td className="px-4 py-3 font-mono text-xs">{n.health.agent_version ?? "—"}</td>
                  <td className="px-4 py-3 text-right">
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
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
    </div>
  );
}
