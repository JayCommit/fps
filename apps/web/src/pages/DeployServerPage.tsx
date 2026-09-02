import { type FormEvent, useEffect, useMemo, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Link, useNavigate } from "react-router-dom";
import { api, ApiError } from "@fps/api-client";
import { GameIcon, inferGameKey } from "../components/GameIcon";
import { EnvEditor, envToRows, rowsToEnv } from "../components/EnvEditor";
import { ErrorBanner, Field, LoadingBlock, PageHeader, Panel, primaryBtn, secondaryBtn } from "../components/PageStates";

export function DeployServerPage() {
  const qc = useQueryClient();
  const navigate = useNavigate();
  const templates = useQuery({ queryKey: ["templates"], queryFn: api.templates });
  const nodes = useQuery({ queryKey: ["nodes"], queryFn: api.nodes });
  const [templateId, setTemplateId] = useState<string>("");
  const [name, setName] = useState("");
  const [envRows, setEnvRows] = useState<{ key: string; value: string }[]>([{ key: "", value: "" }]);
  const [error, setError] = useState<string | null>(null);

  const catalogue = templates.data ?? [];
  const selected = useMemo(
    () => catalogue.find((t) => t.id === templateId) ?? catalogue[0],
    [catalogue, templateId],
  );

  useEffect(() => {
    if (!catalogue.length) return;
    if (!templateId || !catalogue.some((t) => t.id === templateId)) {
      const first = catalogue[0];
      setTemplateId(first.id);
      setEnvRows(envToRows(first.environment ?? {}));
    }
  }, [catalogue, templateId]);

  const create = useMutation({
    mutationFn: api.createServer,
    onSuccess: (server) => {
      qc.invalidateQueries({ queryKey: ["servers"] });
      qc.invalidateQueries({ queryKey: ["dashboard"] });
      navigate(`/servers/${server.id}`);
    },
  });

  function pickTemplate(id: string) {
    setTemplateId(id);
    const tpl = catalogue.find((t) => t.id === id);
    setEnvRows(envToRows(tpl?.environment ?? {}));
    if (!name && tpl) {
      setName(slugName(tpl.slug));
    }
  }

  async function onSubmit(e: FormEvent<HTMLFormElement>) {
    e.preventDefault();
    setError(null);
    const tpl = selected;
    const serverName = name.trim();
    if (!tpl || !serverName) {
      setError("Choose a template and give the server a name.");
      return;
    }
    try {
      await create.mutateAsync({
        name: serverName,
        template_id: tpl.id,
        environment: rowsToEnv(envRows),
      });
    } catch (err) {
      setError(err instanceof ApiError || err instanceof Error ? err.message : "Could not create the server.");
    }
  }

  if (templates.isError) {
    return <ErrorBanner error={templates.error} fallback="Could not load templates." />;
  }
  if (!templates.data) {
    return <LoadingBlock />;
  }

  const dockerReady = (nodes.data ?? []).some((n) => n.health.docker === "available" && n.health.status === "online");

  return (
    <div className="space-y-6">
      <PageHeader
        title="Deploy a server"
        description="Pick a game template, name the instance, then override environment only where you need to."
        actions={
          <Link to="/servers" className={secondaryBtn}>
            Back to servers
          </Link>
        }
      />

      {catalogue.length === 0 ? (
        <p className="text-sm text-[var(--text-muted)]">
          No templates yet.{" "}
          <Link className="text-[var(--accent)]" to="/templates/new">
            Create a native template
          </Link>{" "}
          or import an Egg first.
        </p>
      ) : (
        <form className="space-y-6" onSubmit={onSubmit}>
          <section>
            <h2 className="mb-3 text-sm font-medium uppercase tracking-wide text-[var(--text-faint)]">Template</h2>
            <div className="grid gap-3 sm:grid-cols-2 xl:grid-cols-3">
              {catalogue.map((t) => {
                const active = (selected?.id ?? "") === t.id;
                return (
                  <button
                    type="button"
                    key={t.id}
                    onClick={() => pickTemplate(t.id)}
                    className={`rounded-[var(--radius)] border p-3 text-left ${
                      active
                        ? "border-[var(--accent)] bg-[var(--accent-dim)]"
                        : "ui-card border-[var(--border)] bg-[var(--bg-panel)]"
                    }`}
                  >
                    <div className="flex items-start gap-3">
                      <GameIcon slug={t.slug} name={t.name} game={t.game} />
                      <div className="min-w-0">
                        <div className="font-semibold">{t.name}</div>
                        <div className="mt-0.5 font-mono text-xs text-[var(--text-faint)]">{t.slug}</div>
                      </div>
                    </div>
                    <p className="mt-2 line-clamp-2 text-xs text-[var(--text-muted)]">{t.description}</p>
                    <div className="mt-2 font-mono text-[11px] text-[var(--text-faint)]">
                      {t.memory_mb} MiB · {(t.ports ?? []).length} ports
                    </div>
                  </button>
                );
              })}
            </div>
          </section>

          {selected ? (
            <Panel title="Instance">
              <div className="grid gap-4 lg:grid-cols-[minmax(0,18rem)_1fr]">
                <div className="space-y-3">
                  <Field
                    id="name"
                    label="Server name"
                    required
                    value={name}
                    onChange={(e) => setName(e.target.value)}
                    placeholder={slugName(selected.slug)}
                  />
                  <div className="rounded-[var(--radius)] border border-[var(--border)] bg-[var(--bg)] px-3 py-3 text-sm">
                    <div className="flex items-center gap-2">
                      <GameIcon slug={selected.slug} name={selected.name} game={selected.game} size="sm" />
                      <span className="font-medium">{selected.name}</span>
                    </div>
                    <p className="mt-2 font-mono text-xs text-[var(--text-muted)]">{selected.docker_image}</p>
                    <p className="mt-1 text-xs text-[var(--text-faint)]">
                      {selected.memory_mb} MiB default · game {inferGameKey(selected.slug, selected.name, selected.game)}
                    </p>
                    {!dockerReady ? (
                      <p className="mt-2 text-xs text-[var(--warn)]">
                        No online node with Docker yet. Enroll a game host before deploying.
                      </p>
                    ) : null}
                  </div>
                  <div>
                    <h3 className="mb-1 text-sm text-[var(--text-muted)]">Game ports</h3>
                    {(selected.ports ?? []).length === 0 ? (
                      <p className="text-xs text-[var(--text-faint)]">This template does not declare container ports.</p>
                    ) : (
                      <ul className="space-y-1 font-mono text-xs text-[var(--text-muted)]">
                        {(selected.ports ?? []).map((p) => (
                          <li key={`${p.name}-${p.protocol}-${p.container_port}`}>
                            {p.name} · {p.protocol} · {p.container_port}
                          </li>
                        ))}
                      </ul>
                    )}
                    <p className="mt-2 text-xs text-[var(--text-faint)]">
                      FPS publishes each game port on the matching host port when it is free (Minecraft 25565, CS2 27015,
                      FiveM 30120, …). If that port is taken on the node, the next free port is used and shown on the
                      server page.
                    </p>
                  </div>
                </div>
                <EnvEditor
                  id="environment"
                  label="Environment"
                  hint="Template defaults are prefilled. Change every var you need at deploy time (tokens, EULA, slots, names, RCON). Secrets stay masked."
                  rows={envRows.length ? envRows : envToRows(selected.environment)}
                  onChange={setEnvRows}
                />
              </div>
              {error ? (
                <div className="mt-4">
                  <ErrorBanner error={new Error(error)} fallback={error} />
                </div>
              ) : null}
              <div className="mt-4">
                <button type="submit" disabled={create.isPending} className={primaryBtn}>
                  {create.isPending ? "Scheduling…" : "Deploy server"}
                </button>
              </div>
            </Panel>
          ) : null}
        </form>
      )}
    </div>
  );
}

function slugName(slug: string) {
  return slug.replace(/-/g, " ");
}
