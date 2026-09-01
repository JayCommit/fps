import { type FormEvent, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Link, useParams } from "react-router-dom";
import { api, ApiError } from "@fps/api-client";
import { StatusDot } from "../components/StatusDot";
import {
  dangerBtn,
  EmptyState,
  ErrorBanner,
  Field,
  LoadingBlock,
  Panel,
  primaryBtn,
  secondaryBtn,
  Select,
} from "../components/PageStates";
import { formatBytes, formatWhen, normalizeFiles, statusTone } from "../components/files";

export function ServerDetailPage() {
  const { id } = useParams();
  const qc = useQueryClient();
  const [actionError, setActionError] = useState<string | null>(null);
  const [scheduleError, setScheduleError] = useState<string | null>(null);

  const server = useQuery({
    queryKey: ["server", id],
    queryFn: () => api.server(id!),
    enabled: Boolean(id),
    refetchInterval: 5_000,
  });
  const logs = useQuery({
    queryKey: ["server-logs", id],
    queryFn: () => api.serverLogs(id!),
    enabled: Boolean(id),
    refetchInterval: 3_000,
  });
  const files = useQuery({
    queryKey: ["server-files", id],
    queryFn: () => api.serverFiles(id!),
    enabled: Boolean(id),
  });
  const schedules = useQuery({
    queryKey: ["schedules", id],
    queryFn: () => api.schedules(id),
    enabled: Boolean(id),
  });
  const backups = useQuery({
    queryKey: ["backups", id],
    queryFn: () => api.backups(id),
    enabled: Boolean(id),
  });

  const invalidateServer = () => {
    qc.invalidateQueries({ queryKey: ["server", id] });
    qc.invalidateQueries({ queryKey: ["servers"] });
    qc.invalidateQueries({ queryKey: ["server-logs", id] });
    qc.invalidateQueries({ queryKey: ["backups", id] });
    qc.invalidateQueries({ queryKey: ["dashboard"] });
  };

  const start = useMutation({ mutationFn: () => api.serverStart(id!), onSuccess: invalidateServer });
  const stop = useMutation({ mutationFn: () => api.serverStop(id!), onSuccess: invalidateServer });
  const backup = useMutation({ mutationFn: () => api.serverBackup(id!), onSuccess: invalidateServer });
  const refreshFiles = useMutation({
    mutationFn: () => api.refreshServerFiles(id!),
    onSuccess: () => qc.invalidateQueries({ queryKey: ["server-files", id] }),
  });
  const createSchedule = useMutation({
    mutationFn: api.createSchedule,
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["schedules", id] });
      setScheduleError(null);
    },
  });
  const toggleSchedule = useMutation({
    mutationFn: ({ scheduleId, enabled }: { scheduleId: string; enabled: boolean }) =>
      api.updateSchedule(scheduleId, { enabled }),
    onSuccess: () => qc.invalidateQueries({ queryKey: ["schedules", id] }),
  });

  async function run(action: "start" | "stop" | "backup") {
    setActionError(null);
    try {
      if (action === "start") await start.mutateAsync();
      else if (action === "stop") await stop.mutateAsync();
      else await backup.mutateAsync();
    } catch (err) {
      setActionError(err instanceof ApiError ? err.message : `Could not ${action} this server.`);
    }
  }

  async function onSchedule(e: FormEvent<HTMLFormElement>) {
    e.preventDefault();
    if (!id) return;
    setScheduleError(null);
    const form = new FormData(e.currentTarget);
    const name = String(form.get("name") ?? "").trim();
    const interval_seconds = Number(form.get("interval_seconds"));
    const action = String(form.get("action") ?? "backup") as "start" | "stop" | "backup";
    if (!name || !Number.isFinite(interval_seconds) || interval_seconds <= 0) {
      setScheduleError("Name and a positive interval in seconds are required.");
      return;
    }
    try {
      await createSchedule.mutateAsync({ server_id: id, name, interval_seconds, action });
      e.currentTarget.reset();
    } catch (err) {
      setScheduleError(err instanceof ApiError ? err.message : "Could not create the schedule.");
    }
  }

  if (server.isError) {
    return <ErrorBanner error={server.error} fallback="Server was not found or you do not have permission to view it." />;
  }
  if (!server.data) {
    return <LoadingBlock />;
  }

  const s = server.data;
  const fileList = normalizeFiles(files.data);
  const pending = start.isPending || stop.isPending || backup.isPending;

  return (
    <div className="space-y-6">
      <Link to="/servers" className="text-sm text-[var(--accent)]">
        ← Servers
      </Link>
      <header className="flex flex-wrap items-center justify-between gap-3">
        <div className="flex items-center gap-3">
          <StatusDot status={statusTone(s.status)} />
          <div>
            <h1 className="text-2xl font-semibold">{s.name}</h1>
            <p className="font-mono text-xs text-[var(--text-muted)]">{s.container_name ?? s.id}</p>
          </div>
        </div>
        <div className="flex flex-wrap gap-2">
          <button type="button" className={primaryBtn} disabled={pending} onClick={() => run("start")}>
            {start.isPending ? "Starting…" : "Start"}
          </button>
          <button type="button" className={secondaryBtn} disabled={pending} onClick={() => run("stop")}>
            {stop.isPending ? "Stopping…" : "Stop"}
          </button>
          <button type="button" className={secondaryBtn} disabled={pending} onClick={() => run("backup")}>
            {backup.isPending ? "Snapshotting…" : "Backup"}
          </button>
        </div>
      </header>
      {s.last_error ? (
        <p role="alert" className="text-sm text-[var(--danger)]">
          {s.last_error}
        </p>
      ) : null}
      {actionError ? <ErrorBanner error={new Error(actionError)} fallback={actionError} /> : null}

      <dl className="grid gap-3 sm:grid-cols-2 lg:grid-cols-4">
        <Meta label="Status" value={s.status} />
        <Meta label="Memory" value={`${s.memory_mb} MiB`} />
        <Meta
          label="Node"
          value={s.node_id ?? "unscheduled"}
          href={s.node_id ? `/nodes/${s.node_id}` : undefined}
        />
        <Meta label="Created" value={formatWhen(s.created_at)} />
      </dl>

      <Panel title="Logs">
        {logs.isError ? (
          <ErrorBanner error={logs.error} fallback="Could not load logs." />
        ) : !logs.data ? (
          <LoadingBlock />
        ) : logs.data.length === 0 ? (
          <EmptyState>No log chunks yet. Start the server to stream stdout and stderr from the agent.</EmptyState>
        ) : (
          <pre className="max-h-80 overflow-auto rounded-[var(--radius)] bg-[var(--bg)] p-3 font-mono text-xs leading-5">
            <code>
              {logs.data
                .map((line) => `[${formatWhen(line.created_at)}] ${line.stream}: ${line.chunk.replace(/\n$/, "")}`)
                .join("\n")}
            </code>
          </pre>
        )}
      </Panel>

      <Panel title="Files">
        <div className="mb-3">
          <button
            type="button"
            className={secondaryBtn}
            disabled={refreshFiles.isPending}
            onClick={() => refreshFiles.mutate()}
          >
            {refreshFiles.isPending ? "Refreshing…" : "Refresh listing"}
          </button>
        </div>
        {files.isError ? (
          <ErrorBanner error={files.error} fallback="Could not load files." />
        ) : !files.data && files.isLoading ? (
          <LoadingBlock />
        ) : fileList.length === 0 ? (
          <EmptyState>
            No files reported. Refresh asks the node agent to list the server volume; listings appear after the
            container has been installed.
          </EmptyState>
        ) : (
          <ul className="divide-y divide-[var(--border)] font-mono text-sm">
            {fileList.map((f) => (
              <li key={f.path ?? f.name} className="flex flex-wrap items-center justify-between gap-2 py-2">
                <span>
                  {f.is_dir ? "dir " : "    "}
                  {f.path ?? f.name}
                </span>
                <span className="text-xs text-[var(--text-muted)]">
                  {f.is_dir ? "directory" : formatBytes(f.size ?? null)}
                  {f.modified_at ? ` · ${formatWhen(f.modified_at)}` : ""}
                </span>
              </li>
            ))}
          </ul>
        )}
      </Panel>

      <Panel title="Schedules">
        <form className="mb-4 grid gap-3 sm:grid-cols-4" onSubmit={onSchedule}>
          <Field id="name" label="Name" required placeholder="Nightly backup" />
          <Field
            id="interval_seconds"
            label="Interval (seconds)"
            type="number"
            min={30}
            defaultValue={3600}
            required
            hint="3600 is hourly."
          />
          <Select id="action" label="Action" defaultValue="backup">
            <option value="start">start</option>
            <option value="stop">stop</option>
            <option value="backup">backup</option>
          </Select>
          <div className="flex items-end">
            <button type="submit" className={primaryBtn} disabled={createSchedule.isPending}>
              {createSchedule.isPending ? "Saving…" : "Add schedule"}
            </button>
          </div>
        </form>
        {scheduleError ? <ErrorBanner error={new Error(scheduleError)} fallback={scheduleError} /> : null}
        {schedules.isError ? (
          <ErrorBanner error={schedules.error} fallback="Could not load schedules." />
        ) : !schedules.data ? (
          <LoadingBlock />
        ) : schedules.data.length === 0 ? (
          <p className="text-sm text-[var(--text-muted)]">No schedules. Add one to start, stop, or back up on an interval.</p>
        ) : (
          <ul className="space-y-2">
            {schedules.data.map((sch) => (
              <li
                key={sch.id}
                className="flex flex-wrap items-center justify-between gap-3 rounded-[var(--radius)] border border-[var(--border)] px-3 py-2"
              >
                <div>
                  <div className="font-medium">{sch.name}</div>
                  <div className="text-xs text-[var(--text-muted)]">
                    {sch.action} every {sch.interval_seconds}s · last {formatWhen(sch.last_run_at)} · next{" "}
                    {formatWhen(sch.next_run_at)}
                  </div>
                </div>
                <button
                  type="button"
                  className={sch.enabled ? dangerBtn : secondaryBtn}
                  disabled={toggleSchedule.isPending}
                  onClick={() => toggleSchedule.mutate({ scheduleId: sch.id, enabled: !sch.enabled })}
                >
                  {sch.enabled ? "Disable" : "Enable"}
                </button>
              </li>
            ))}
          </ul>
        )}
      </Panel>

      <Panel title="Backups">
        {backups.isError ? (
          <ErrorBanner error={backups.error} fallback="Could not load backups." />
        ) : !backups.data ? (
          <LoadingBlock />
        ) : backups.data.length === 0 ? (
          <p className="text-sm text-[var(--text-muted)]">No backups for this server yet.</p>
        ) : (
          <ul className="space-y-2 text-sm">
            {backups.data.map((b) => (
              <li key={b.id} className="flex flex-wrap items-center justify-between gap-2">
                <span className="inline-flex items-center gap-2">
                  <StatusDot status={statusTone(b.status)} />
                  {b.status}
                  <span className="font-mono text-xs text-[var(--text-muted)]">{formatWhen(b.created_at)}</span>
                </span>
                <span className="font-mono text-xs text-[var(--text-muted)]">
                  {b.archive_path ?? "pending"} · {formatBytes(b.size_bytes)}
                </span>
              </li>
            ))}
          </ul>
        )}
      </Panel>
    </div>
  );
}

function Meta({ label, value, href }: { label: string; value: string; href?: string }) {
  return (
    <div className="rounded-[var(--radius)] border border-[var(--border)] bg-[var(--bg-panel)] px-4 py-3">
      <dt className="text-xs uppercase tracking-wide text-[var(--text-faint)]">{label}</dt>
      <dd className="mt-1 break-all">
        {href ? (
          <Link className="text-[var(--accent)]" to={href}>
            {value}
          </Link>
        ) : (
          value
        )}
      </dd>
    </div>
  );
}
