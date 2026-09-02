import { type FormEvent, useEffect, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Link, useNavigate, useParams } from "react-router-dom";
import { api, ApiError } from "@fps/api-client";
import { StatusDot } from "../components/StatusDot";
import {
  EmptyState,
  ErrorBanner,
  Field,
  LoadingBlock,
  Panel,
  primaryBtn,
  secondaryBtn,
  dangerBtn,
  Select,
  TextArea,
} from "../components/PageStates";
import { GameIcon } from "../components/GameIcon";
import { AddonsPanel } from "../components/AddonsPanel";
import { formatBytes, formatWhen, normalizeFiles, statusTone } from "../components/files";
import { EnvEditor, envToRows, rowsToEnv } from "../components/EnvEditor";
import { formatAllocatedPort } from "../components/ports";
import { Sparkline } from "../components/Sparkline";
import { LiveConsole } from "./LiveConsole";

export function ServerDetailPage() {
  const { id } = useParams();
  const qc = useQueryClient();
  const navigate = useNavigate();
  const [actionError, setActionError] = useState<string | null>(null);
  const [scheduleError, setScheduleError] = useState<string | null>(null);
  const [filePath, setFilePath] = useState("");
  const [fileBody, setFileBody] = useState("");
  const [fileError, setFileError] = useState<string | null>(null);
  const [envRows, setEnvRows] = useState<{ key: string; value: string }[]>([{ key: "", value: "" }]);
  const [serverName, setServerName] = useState("");
  const [envHydratedFor, setEnvHydratedFor] = useState<string | null>(null);
  const [configError, setConfigError] = useState<string | null>(null);

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
  const templates = useQuery({ queryKey: ["templates"], queryFn: api.templates });
  const samples = useQuery({
    queryKey: ["server-metrics", id],
    queryFn: () => api.serverMetrics(id!),
    enabled: Boolean(id),
    refetchInterval: 15_000,
  });

  useEffect(() => {
    if (!id || !server.data || envHydratedFor === id) return;
    setEnvHydratedFor(id);
    setServerName(server.data.name);
    setEnvRows(envToRows(asEnv(server.data.environment)));
  }, [id, server.data, envHydratedFor]);

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
  const remove = useMutation({
    mutationFn: () => api.deleteServer(id!),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["servers"] });
      qc.invalidateQueries({ queryKey: ["dashboard"] });
      navigate("/servers");
    },
  });
  const patch = useMutation({
    mutationFn: (body: { name?: string; environment?: Record<string, string> }) => api.patchServer(id!, body),
    onSuccess: (updated) => {
      qc.setQueryData(["server", id], updated);
      invalidateServer();
      setServerName(updated.name);
      setEnvRows(envToRows(asEnv(updated.environment)));
    },
  });
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
  const pending = start.isPending || stop.isPending || backup.isPending || remove.isPending;
  const busyStatus = s.status === "installing" || s.status === "deleting";
  const tpl = templates.data?.find((t) => t.id === s.template_id);
  const ports = s.ports ?? [];

  return (
    <div className="space-y-6">
      <Link to="/servers" className="text-sm text-[var(--accent)]">
        ← Servers
      </Link>
      <header className="flex flex-wrap items-center justify-between gap-3">
        <div className="flex items-center gap-3">
          <GameIcon slug={tpl?.slug} name={tpl?.name ?? s.name} game={tpl?.game} size="lg" />
          <div>
            <div className="flex items-center gap-2">
              <StatusDot status={statusTone(s.status)} />
              <h1 className="text-2xl font-semibold">{s.name}</h1>
            </div>
            <p className="font-mono text-xs text-[var(--text-muted)]">
              {tpl?.name ?? "template"} · {s.container_name ?? s.id}
            </p>
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
          <button
            type="button"
            className={dangerBtn}
            disabled={pending || s.status === "deleting"}
            onClick={async () => {
              if (!window.confirm(`Delete server “${s.name}”? This cannot be undone.`)) return;
              setActionError(null);
              try {
                await remove.mutateAsync();
              } catch (err) {
                setActionError(err instanceof ApiError ? err.message : "Could not delete this server.");
              }
            }}
          >
            {remove.isPending ? "Deleting…" : "Delete"}
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
        <Meta label="Restarts" value={String(s.restart_count ?? 0)} />
      </dl>

      <Panel title="Published ports">
        {ports.length === 0 ? (
          <EmptyState>
            {busyStatus
              ? "Ports appear here after install finishes. Watch the live console for pull and start output."
              : "No published ports yet."}
          </EmptyState>
        ) : (
          <ul className="space-y-2 font-mono text-sm">
            {ports.map((p) => (
              <li
                key={`${p.name}-${p.protocol}-${p.host_port}-${p.container_port}`}
                className="rounded-[var(--radius)] border border-[var(--border)] bg-[var(--bg)] px-3 py-2"
              >
                {formatAllocatedPort(p)}
              </li>
            ))}
          </ul>
        )}
      </Panel>

      <Panel title="Environment">
        <form
          className="space-y-4"
          onSubmit={async (e) => {
            e.preventDefault();
            if (!id) return;
            setConfigError(null);
            try {
              await patch.mutateAsync({
                name: serverName.trim() || undefined,
                environment: rowsToEnv(envRows),
              });
            } catch (err) {
              setConfigError(err instanceof ApiError ? err.message : "Could not save environment.");
            }
          }}
        >
          <Field
            id="server_name"
            label="Server name"
            value={serverName}
            onChange={(e) => setServerName(e.target.value)}
          />
          <EnvEditor
            id="environment"
            label="Environment"
            hint="Saving applies on the next container recreate; FPS reinstalls the workload with the new env."
            rows={envRows}
            onChange={setEnvRows}
          />
          {configError ? <ErrorBanner error={new Error(configError)} fallback={configError} /> : null}
          <button type="submit" className={primaryBtn} disabled={patch.isPending}>
            {patch.isPending ? "Saving…" : "Save environment"}
          </button>
        </form>
      </Panel>

      {id ? <AddonsPanel serverId={id} /> : null}

      <Panel title="Resources">
        {samples.data && samples.data.length > 0 ? (
          <div className="grid gap-3 lg:grid-cols-2">
            <Sparkline
              label="Memory (bytes)"
              values={samples.data.map((p) => p.memory_bytes ?? 0)}
            />
            <Sparkline
              label="CPU percent"
              values={samples.data.map((p) => p.cpu_percent ?? 0)}
            />
          </div>
        ) : (
          <EmptyState>Heartbeat samples appear here after the agent reports container stats.</EmptyState>
        )}
      </Panel>

      <Panel
        title={busyStatus ? "Live console — install" : "Live console"}
      >
        {busyStatus ? (
          <p className="mb-3 text-xs text-[var(--text-muted)]">
            Operators can watch image pull and install output here while the node prepares or tears down the container.
          </p>
        ) : null}
        {id ? <LiveConsole serverId={id} status={s.status} /> : null}
      </Panel>

      <Panel title="Logs">
        {logs.isError ? (
          <ErrorBanner error={logs.error} fallback="Could not load logs." />
        ) : !logs.data ? (
          <LoadingBlock />
        ) : logs.data.length === 0 ? (
          <EmptyState>
            {busyStatus
              ? "No log chunks yet. Pull and install output streams in the live console as the agent works."
              : "No log chunks yet. Start the server to stream stdout and stderr from the agent."}
          </EmptyState>
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
                <button
                  type="button"
                  className="text-left text-[var(--accent)]"
                  onClick={() => setFilePath(f.path ?? f.name)}
                >
                  {f.is_dir ? "dir " : "    "}
                  {f.path ?? f.name}
                </button>
                <span className="text-xs text-[var(--text-muted)]">
                  {f.is_dir ? "directory" : formatBytes(f.size ?? null)}
                  {f.modified_at ? ` · ${formatWhen(f.modified_at)}` : ""}
                </span>
              </li>
            ))}
          </ul>
        )}
        <form
          className="mt-4 space-y-3"
          onSubmit={async (e) => {
            e.preventDefault();
            if (!id || !filePath) return;
            setFileError(null);
            try {
              const job = await api.writeServerFile(id, filePath, fileBody);
              setFileError(`Write queued (${job.id}).`);
            } catch (err) {
              setFileError(err instanceof ApiError ? err.message : "Could not write the file.");
            }
          }}
        >
          <Field
            id="file_path"
            label="Path"
            value={filePath}
            onChange={(e) => setFilePath(e.target.value)}
            placeholder="eula.txt"
          />
          <TextArea
            id="file_body"
            label="Contents"
            value={fileBody}
            onChange={(e) => setFileBody(e.target.value)}
          />
          <div className="flex gap-2">
            <button
              type="button"
              className={secondaryBtn}
              onClick={async () => {
                if (!id || !filePath) return;
                setFileError(null);
                try {
                  const job = await api.readServerFile(id, filePath);
                  for (let i = 0; i < 20; i++) {
                    await new Promise((r) => setTimeout(r, 500));
                    const seen = await api.job(job.id);
                    if (seen.status === "succeeded" && seen.result?.file_content != null) {
                      setFileBody(seen.result.file_content);
                      setFileError(null);
                      return;
                    }
                    if (seen.status === "failed") {
                      setFileError(seen.result?.message ?? "Read failed.");
                      return;
                    }
                  }
                  const cached = await api.server(id);
                  if (cached.last_file?.content != null) {
                    setFileBody(cached.last_file.content);
                  } else {
                    setFileError("Read queued. Wait one heartbeat and try again.");
                  }
                } catch (err) {
                  setFileError(err instanceof ApiError ? err.message : "Could not read the file.");
                }
              }}
            >
              Read
            </button>
            <button type="submit" className={primaryBtn}>
              Write
            </button>
          </div>
          {fileError ? <p className="text-sm text-[var(--text-muted)]">{fileError}</p> : null}
        </form>
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
                <button
                  type="button"
                  className={secondaryBtn}
                  disabled={b.status !== "succeeded"}
                  onClick={async () => {
                    setActionError(null);
                    try {
                      await api.restoreBackup(b.id);
                      invalidateServer();
                    } catch (err) {
                      setActionError(err instanceof ApiError ? err.message : "Could not restore.");
                    }
                  }}
                >
                  Restore
                </button>
              </li>
            ))}
          </ul>
        )}
      </Panel>
    </div>
  );
}

function asEnv(environment: unknown): Record<string, string> {
  if (!environment || typeof environment !== "object" || Array.isArray(environment)) return {};
  const out: Record<string, string> = {};
  for (const [key, value] of Object.entries(environment as Record<string, unknown>)) {
    out[key] = value == null ? "" : String(value);
  }
  return out;
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
