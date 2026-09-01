import { type FormEvent, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Link, useNavigate } from "react-router-dom";
import { api, ApiError } from "@fps/api-client";
import { StatusDot } from "../components/StatusDot";
import { EmptyState, ErrorBanner, Field, LoadingBlock, Panel, primaryBtn, Select, TextArea } from "../components/PageStates";
import { parseEnvironment } from "../components/envFormat";
import { formatWhen, statusTone } from "../components/files";

export function ServersPage() {
  const qc = useQueryClient();
  const navigate = useNavigate();
  const servers = useQuery({ queryKey: ["servers"], queryFn: api.servers, refetchInterval: 5_000 });
  const templates = useQuery({ queryKey: ["templates"], queryFn: api.templates });
  const [error, setError] = useState<string | null>(null);

  const create = useMutation({
    mutationFn: api.createServer,
    onSuccess: (server) => {
      qc.invalidateQueries({ queryKey: ["servers"] });
      qc.invalidateQueries({ queryKey: ["dashboard"] });
      navigate(`/servers/${server.id}`);
    },
  });

  async function onSubmit(e: FormEvent<HTMLFormElement>) {
    e.preventDefault();
    setError(null);
    const form = new FormData(e.currentTarget);
    const name = String(form.get("name") ?? "").trim();
    const template_id = String(form.get("template_id") ?? "").trim();
    if (!name || !template_id) {
      setError("Choose a template and give the server a name.");
      return;
    }
    try {
      const environment = parseEnvironment(String(form.get("environment") ?? ""));
      await create.mutateAsync({ name, template_id, environment });
    } catch (err) {
      setError(err instanceof ApiError || err instanceof Error ? err.message : "Could not create the server.");
    }
  }

  if (servers.isError) {
    return <ErrorBanner error={servers.error} fallback="Could not load servers." />;
  }

  const catalogue = templates.data ?? [];

  return (
    <div className="space-y-6">
      <header>
        <h1 className="text-2xl font-semibold">Game servers</h1>
        <p className="text-[var(--text-muted)]">
          Deploy from a template. The control plane schedules the workload onto an enrolled node with Docker ready.
        </p>
      </header>

      <Panel title="Deploy">
        {templates.isError ? (
          <ErrorBanner error={templates.error} fallback="Could not load templates." />
        ) : !templates.data ? (
          <LoadingBlock />
        ) : catalogue.length === 0 ? (
          <p className="text-sm text-[var(--text-muted)]">
            No templates yet.{" "}
            <Link className="text-[var(--accent)] underline" to="/templates">
              Add a native template or import an Egg
            </Link>{" "}
            before deploying.
          </p>
        ) : (
          <form className="grid gap-3 sm:grid-cols-2" onSubmit={onSubmit}>
            <Select id="template_id" label="Template" required>
              {catalogue.map((t) => (
                <option key={t.id} value={t.id}>
                  {t.name} ({t.slug})
                </option>
              ))}
            </Select>
            <Field id="name" label="Server name" required placeholder="survival-overworld" />
            <div className="sm:col-span-2">
              <TextArea
                id="environment"
                label="Environment (optional)"
                hint="JSON object or KEY=value lines. Values override the template defaults when the node installs the container."
                placeholder={"EULA=true\nDIFFICULTY=normal"}
              />
            </div>
            {error ? (
              <div className="sm:col-span-2">
                <ErrorBanner error={new Error(error)} fallback={error} />
              </div>
            ) : null}
            <div className="sm:col-span-2">
              <button type="submit" disabled={create.isPending} className={primaryBtn}>
                {create.isPending ? "Scheduling…" : "Deploy server"}
              </button>
            </div>
          </form>
        )}
      </Panel>

      {!servers.data ? (
        <LoadingBlock />
      ) : servers.data.length === 0 ? (
        <EmptyState>No game servers yet. Use the form above to schedule one onto an enrolled node.</EmptyState>
      ) : (
        <div className="overflow-x-auto rounded-[var(--radius)] border border-[var(--border)]">
          <table className="w-full text-left text-sm">
            <thead className="bg-[var(--bg-raised)] text-xs uppercase tracking-wide text-[var(--text-faint)]">
              <tr>
                <th className="px-4 py-2">Server</th>
                <th className="px-4 py-2">Status</th>
                <th className="px-4 py-2">Memory</th>
                <th className="px-4 py-2">Node</th>
                <th className="px-4 py-2">Created</th>
              </tr>
            </thead>
            <tbody>
              {servers.data.map((s) => (
                <tr key={s.id} className="border-t border-[var(--border)]">
                  <td className="px-4 py-3">
                    <Link className="font-medium text-[var(--accent)]" to={`/servers/${s.id}`}>
                      {s.name}
                    </Link>
                    <div className="font-mono text-xs text-[var(--text-muted)]">{s.container_name ?? s.id}</div>
                    {s.last_error ? <div className="mt-1 text-xs text-[var(--danger)]">{s.last_error}</div> : null}
                  </td>
                  <td className="px-4 py-3">
                    <span className="inline-flex items-center gap-2">
                      <StatusDot status={statusTone(s.status)} />
                      {s.status}
                    </span>
                  </td>
                  <td className="px-4 py-3 font-mono text-xs">{s.memory_mb} MiB</td>
                  <td className="px-4 py-3 font-mono text-xs">
                    {s.node_id ? (
                      <Link className="text-[var(--accent)]" to={`/nodes/${s.node_id}`}>
                        {s.node_id.slice(0, 8)}…
                      </Link>
                    ) : (
                      "unscheduled"
                    )}
                  </td>
                  <td className="px-4 py-3 font-mono text-xs">{formatWhen(s.created_at)}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
    </div>
  );
}
