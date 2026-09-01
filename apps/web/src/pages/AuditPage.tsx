import { useQuery } from "@tanstack/react-query";
import { api } from "@fps/api-client";
import { EmptyState, ErrorBanner, LoadingBlock } from "../components/PageStates";
import { formatWhen } from "../components/files";

export function AuditPage() {
  const audit = useQuery({ queryKey: ["audit"], queryFn: api.audit, refetchInterval: 10_000 });

  if (audit.isError) {
    return <ErrorBanner error={audit.error} fallback="Could not load the audit log." />;
  }
  if (!audit.data) {
    return <LoadingBlock />;
  }

  return (
    <div className="space-y-6">
      <header>
        <h1 className="text-2xl font-semibold">Audit</h1>
        <p className="text-[var(--text-muted)]">
          Immutable control-plane events. Actor, resource, and details are recorded for every privileged action.
        </p>
      </header>
      {audit.data.length === 0 ? (
        <EmptyState>No audit events yet. Enrollment, invites, and server actions will appear here.</EmptyState>
      ) : (
        <div className="overflow-x-auto rounded-[var(--radius)] border border-[var(--border)]">
          <table className="w-full text-left text-sm">
            <thead className="bg-[var(--bg-raised)] text-xs uppercase tracking-wide text-[var(--text-faint)]">
              <tr>
                <th className="px-4 py-2">When</th>
                <th className="px-4 py-2">Action</th>
                <th className="px-4 py-2">Resource</th>
                <th className="px-4 py-2">Actor</th>
                <th className="px-4 py-2">Details</th>
              </tr>
            </thead>
            <tbody>
              {audit.data.map((ev) => (
                <tr key={ev.id} className="border-t border-[var(--border)] align-top">
                  <td className="whitespace-nowrap px-4 py-3 font-mono text-xs">{formatWhen(ev.created_at)}</td>
                  <td className="px-4 py-3 font-mono text-xs">{ev.action}</td>
                  <td className="px-4 py-3">
                    <div>{ev.resource_type}</div>
                    <div className="font-mono text-xs text-[var(--text-muted)]">{ev.resource_id ?? "—"}</div>
                  </td>
                  <td className="px-4 py-3 font-mono text-xs">{ev.actor_user_id ?? "system"}</td>
                  <td className="px-4 py-3 font-mono text-xs text-[var(--text-muted)]">
                    {ev.details == null || ev.details === ""
                      ? "—"
                      : typeof ev.details === "string"
                        ? ev.details
                        : JSON.stringify(ev.details)}
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
