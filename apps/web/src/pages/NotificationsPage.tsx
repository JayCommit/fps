import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { api } from "@fps/api-client";
import { EmptyState, ErrorBanner, LoadingBlock, secondaryBtn } from "../components/PageStates";
import { formatWhen } from "../components/files";

export function NotificationsPage() {
  const qc = useQueryClient();
  const notes = useQuery({
    queryKey: ["notifications"],
    queryFn: api.notifications,
    refetchInterval: 10_000,
  });
  const markRead = useMutation({
    mutationFn: api.markNotificationRead,
    onSuccess: () => qc.invalidateQueries({ queryKey: ["notifications"] }),
  });

  if (notes.isError) {
    return <ErrorBanner error={notes.error} fallback="Could not load notifications." />;
  }
  if (!notes.data) {
    return <LoadingBlock />;
  }

  return (
    <div className="space-y-6">
      <header>
        <h1 className="text-2xl font-semibold">Notifications</h1>
        <p className="text-[var(--text-muted)]">
          Job failures, node health, and backup outcomes. Marking read is stored on the control plane.
        </p>
      </header>
      {notes.data.length === 0 ? (
        <EmptyState>No notifications. Failed jobs and degraded nodes will land here.</EmptyState>
      ) : (
        <ul className="space-y-2">
          {notes.data.map((n) => (
            <li
              key={n.id}
              className={`rounded-[var(--radius)] border border-[var(--border)] bg-[var(--bg-panel)] px-4 py-3 ${
                n.read_at ? "opacity-70" : ""
              }`}
            >
              <div className="flex flex-wrap items-start justify-between gap-3">
                <div>
                  <div className="text-xs uppercase tracking-wide text-[var(--text-faint)]">{n.kind}</div>
                  <h2 className="font-medium">{n.title}</h2>
                  <p className="mt-1 text-sm text-[var(--text-muted)]">{n.body}</p>
                  <p className="mt-1 font-mono text-xs text-[var(--text-faint)]">{formatWhen(n.created_at)}</p>
                </div>
                {n.read_at ? (
                  <span className="text-xs text-[var(--text-faint)]">Read {formatWhen(n.read_at)}</span>
                ) : (
                  <button
                    type="button"
                    className={secondaryBtn}
                    disabled={markRead.isPending}
                    onClick={() => markRead.mutate(n.id)}
                  >
                    Mark read
                  </button>
                )}
              </div>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}
