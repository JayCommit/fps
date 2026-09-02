import { Link } from "react-router-dom";
import { useQuery } from "@tanstack/react-query";
import { Plus } from "lucide-react";
import { api } from "@fps/api-client";
import { StatusDot } from "../components/StatusDot";
import { GameIcon } from "../components/GameIcon";
import { EmptyState, ErrorBanner, LoadingBlock, PageHeader, primaryBtn } from "../components/PageStates";
import { formatRelative, statusTone } from "../components/files";
import { formatAllocatedPort, primaryAllocatedPort } from "../components/ports";

export function ServersPage() {
  const servers = useQuery({ queryKey: ["servers"], queryFn: api.servers, refetchInterval: 5_000 });
  const templates = useQuery({ queryKey: ["templates"], queryFn: api.templates });
  const nodes = useQuery({ queryKey: ["nodes"], queryFn: api.nodes, refetchInterval: 8_000 });

  if (servers.isError) {
    return <ErrorBanner error={servers.error} fallback="Could not load servers." />;
  }

  const tplById = new Map((templates.data ?? []).map((t) => [t.id, t]));
  const nodeById = new Map((nodes.data ?? []).map((n) => [n.id, n]));

  return (
    <div className="space-y-6">
      <PageHeader
        title="Game servers"
        description="Workloads scheduled onto enrolled nodes. Deploy from a template — environment overrides are per-server."
        actions={
          <Link to="/servers/new" className={primaryBtn}>
            <Plus size={16} /> Deploy server
          </Link>
        }
      />

      {!servers.data ? (
        <LoadingBlock />
      ) : servers.data.length === 0 ? (
        <EmptyState>
          No game servers yet.{" "}
          <Link className="text-[var(--accent)]" to="/servers/new">
            Deploy from the catalogue
          </Link>{" "}
          once a node with Docker is online.
        </EmptyState>
      ) : (
        <div className="grid gap-3 md:grid-cols-2 xl:grid-cols-3">
          {servers.data.map((s) => {
            const tpl = tplById.get(s.template_id);
            const node = s.node_id ? nodeById.get(s.node_id) : undefined;
            const primaryPort = primaryAllocatedPort(s.ports);
            return (
              <Link
                key={s.id}
                to={`/servers/${s.id}`}
                className="ui-card rounded-[var(--radius)] border border-[var(--border)] bg-[var(--bg-panel)] p-4"
              >
                <div className="flex items-start gap-3">
                  <GameIcon slug={tpl?.slug} name={tpl?.name ?? s.name} game={tpl?.game} />
                  <div className="min-w-0 flex-1">
                    <div className="flex items-start justify-between gap-2">
                      <h2 className="truncate font-semibold">{s.name}</h2>
                      <span className="inline-flex items-center gap-1.5 rounded-full border border-[var(--border)] px-2 py-0.5 text-xs">
                        <StatusDot status={statusTone(s.status)} />
                        {s.status}
                      </span>
                    </div>
                    <p className="mt-1 truncate text-sm text-[var(--text-muted)]">{tpl?.name ?? "Unknown template"}</p>
                  </div>
                </div>
                <dl className="mt-4 grid grid-cols-2 gap-2 font-mono text-xs text-[var(--text-muted)]">
                  <div>
                    <dt className="text-[var(--text-faint)]">Memory</dt>
                    <dd>{s.memory_mb} MiB</dd>
                  </div>
                  <div>
                    <dt className="text-[var(--text-faint)]">Node</dt>
                    <dd className="truncate">{node?.name ?? (s.node_id ? s.node_id.slice(0, 8) : "unscheduled")}</dd>
                  </div>
                  <div className="col-span-2">
                    <dt className="text-[var(--text-faint)]">Created</dt>
                    <dd>{formatRelative(s.created_at)}</dd>
                  </div>
                  {primaryPort ? (
                    <div className="col-span-2">
                      <dt className="text-[var(--text-faint)]">Port</dt>
                      <dd>{formatAllocatedPort(primaryPort)}</dd>
                    </div>
                  ) : null}
                </dl>
                {s.last_error ? <p className="mt-3 text-xs text-[var(--danger)]">{s.last_error}</p> : null}
              </Link>
            );
          })}
        </div>
      )}
    </div>
  );
}
