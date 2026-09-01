import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Link, useNavigate, useParams } from "react-router-dom";
import { useState } from "react";
import { api, ApiError } from "@fps/api-client";
import { StatusDot } from "../components/StatusDot";
import { dangerBtn } from "../components/PageStates";

export function NodeDetailPage() {
  const { id } = useParams();
  const navigate = useNavigate();
  const qc = useQueryClient();
  const [revokeError, setRevokeError] = useState<string | null>(null);
  const node = useQuery({
    queryKey: ["node", id],
    queryFn: () => api.node(id!),
    enabled: Boolean(id),
    refetchInterval: 5_000,
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
    return <p role="alert">Node was not found or you do not have permission to view it.</p>;
  }
  if (!node.data) {
    return <div className="h-40 animate-pulse rounded bg-[var(--bg-hover)]" aria-busy="true" />;
  }
  const n = node.data;
  return (
    <div className="space-y-4">
      <Link to="/nodes" className="text-sm text-[var(--accent)]">
        ← Nodes
      </Link>
      <header className="flex flex-wrap items-center justify-between gap-3">
        <div className="flex items-center gap-3">
          <StatusDot status={n.health.status} />
          <h1 className="text-2xl font-semibold">{n.name}</h1>
        </div>
        <button
          type="button"
          className={dangerBtn}
          disabled={revoke.isPending}
          onClick={async () => {
            setRevokeError(null);
            try {
              await revoke.mutateAsync();
            } catch (err) {
              setRevokeError(err instanceof ApiError ? err.message : "Could not revoke this node.");
            }
          }}
        >
          {revoke.isPending ? "Revoking…" : "Revoke enrollment"}
        </button>
      </header>
      <p className="text-[var(--text-muted)]">{n.health.message}</p>
      {revokeError ? (
        <p role="alert" className="text-sm text-[var(--danger)]">
          {revokeError}
        </p>
      ) : null}
      <dl className="grid gap-3 sm:grid-cols-2">
        <Item label="Hostname" value={n.hostname} />
        <Item label="Architecture" value={n.architecture ?? "—"} />
        <Item label="OS" value={n.operating_system ?? "—"} />
        <Item label="Docker" value={n.health.docker} />
        <Item label="Agent" value={n.health.agent_version ?? "—"} />
        <Item label="Protocol" value={String(n.health.protocol_version)} />
        <Item
          label="CPU / memory"
          value={`${n.health.resources.cpu_cores ?? "?"} cores · ${formatBytes(n.health.resources.memory_bytes)}`}
        />
        <Item label="Node ID" value={n.id} mono />
      </dl>
    </div>
  );
}

function Item({ label, value, mono }: { label: string; value: string; mono?: boolean }) {
  return (
    <div className="rounded-[var(--radius)] border border-[var(--border)] bg-[var(--bg-panel)] px-4 py-3">
      <dt className="text-xs uppercase tracking-wide text-[var(--text-faint)]">{label}</dt>
      <dd className={`mt-1 ${mono ? "font-mono text-xs break-all" : ""}`}>{value}</dd>
    </div>
  );
}

function formatBytes(n?: number) {
  if (!n) return "—";
  const gib = n / 1024 / 1024 / 1024;
  return `${gib.toFixed(1)} GiB`;
}
