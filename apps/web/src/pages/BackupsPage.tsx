import { useQuery } from "@tanstack/react-query";
import { Link, useSearchParams } from "react-router-dom";
import { api } from "@fps/api-client";
import { StatusDot } from "../components/StatusDot";
import { EmptyState, ErrorBanner, LoadingBlock } from "../components/PageStates";
import { formatBytes, formatWhen, statusTone } from "../components/files";

export function BackupsPage() {
  const [params] = useSearchParams();
  const serverId = params.get("server_id") ?? undefined;
  const backups = useQuery({
    queryKey: ["backups", serverId ?? "all"],
    queryFn: () => api.backups(serverId),
    refetchInterval: 8_000,
  });
  const servers = useQuery({ queryKey: ["servers"], queryFn: api.servers });

  if (backups.isError) {
    return <ErrorBanner error={backups.error} fallback="Could not load backups." />;
  }
  if (!backups.data) {
    return <LoadingBlock />;
  }

  const names = new Map((servers.data ?? []).map((s) => [s.id, s.name]));

  return (
    <div className="space-y-6">
      <header>
        <h1 className="text-2xl font-semibold">Backups</h1>
        <p className="text-[var(--text-muted)]">
          Archives produced by start/stop/backup jobs on game nodes. Trigger a snapshot from a server detail page.
        </p>
      </header>
      {serverId ? (
        <p className="text-sm text-[var(--text-muted)]">
          Filtered to server {names.get(serverId) ?? serverId}.{" "}
          <Link className="text-[var(--accent)] underline" to="/backups">
            Show all
          </Link>
        </p>
      ) : null}
      {backups.data.length === 0 ? (
        <EmptyState>
          No backups yet. Open a{" "}
          <Link className="text-[var(--accent)] underline" to="/servers">
            server
          </Link>{" "}
          and use Backup, or add a backup schedule.
        </EmptyState>
      ) : (
        <div className="overflow-x-auto rounded-[var(--radius)] border border-[var(--border)]">
          <table className="w-full text-left text-sm">
            <thead className="bg-[var(--bg-raised)] text-xs uppercase tracking-wide text-[var(--text-faint)]">
              <tr>
                <th className="px-4 py-2">Server</th>
                <th className="px-4 py-2">Status</th>
                <th className="px-4 py-2">Size</th>
                <th className="px-4 py-2">Archive</th>
                <th className="px-4 py-2">Created</th>
              </tr>
            </thead>
            <tbody>
              {backups.data.map((b) => (
                <tr key={b.id} className="border-t border-[var(--border)]">
                  <td className="px-4 py-3">
                    <Link className="text-[var(--accent)]" to={`/servers/${b.server_id}`}>
                      {names.get(b.server_id) ?? b.server_id}
                    </Link>
                  </td>
                  <td className="px-4 py-3">
                    <span className="inline-flex items-center gap-2">
                      <StatusDot status={statusTone(b.status)} />
                      {b.status}
                    </span>
                    {b.error ? <div className="text-xs text-[var(--danger)]">{b.error}</div> : null}
                  </td>
                  <td className="px-4 py-3 font-mono text-xs">{formatBytes(b.size_bytes)}</td>
                  <td className="px-4 py-3 font-mono text-xs">{b.archive_path ?? "—"}</td>
                  <td className="px-4 py-3 font-mono text-xs">{formatWhen(b.created_at)}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
    </div>
  );
}
