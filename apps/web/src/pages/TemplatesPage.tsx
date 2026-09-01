import { type FormEvent, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { api, ApiError } from "@fps/api-client";
import { EmptyState, ErrorBanner, Field, LoadingBlock, Panel, primaryBtn, TextArea } from "../components/PageStates";
import { parseEnvironment, parsePorts } from "../components/envFormat";

export function TemplatesPage() {
  const qc = useQueryClient();
  const templates = useQuery({ queryKey: ["templates"], queryFn: api.templates });
  const [nativeError, setNativeError] = useState<string | null>(null);
  const [eggError, setEggError] = useState<string | null>(null);

  const create = useMutation({
    mutationFn: api.createTemplate,
    onSuccess: () => qc.invalidateQueries({ queryKey: ["templates"] }),
  });
  const importEgg = useMutation({
    mutationFn: api.importEgg,
    onSuccess: () => qc.invalidateQueries({ queryKey: ["templates"] }),
  });

  async function onCreate(e: FormEvent<HTMLFormElement>) {
    e.preventDefault();
    setNativeError(null);
    const form = new FormData(e.currentTarget);
    try {
      await create.mutateAsync({
        name: String(form.get("name") ?? "").trim(),
        slug: String(form.get("slug") ?? "").trim(),
        description: String(form.get("description") ?? "").trim(),
        docker_image: String(form.get("docker_image") ?? "").trim(),
        memory_mb: Number(form.get("memory_mb") || 64),
        startup: String(form.get("startup") ?? "").trim() || undefined,
        environment: parseEnvironment(String(form.get("environment") ?? "")),
        ports: parsePorts(String(form.get("ports") ?? "")),
      });
      e.currentTarget.reset();
    } catch (err) {
      setNativeError(err instanceof ApiError || err instanceof Error ? err.message : "Could not create the template.");
    }
  }

  async function onImport(e: FormEvent<HTMLFormElement>) {
    e.preventDefault();
    setEggError(null);
    const form = new FormData(e.currentTarget);
    const raw = String(form.get("egg") ?? "").trim();
    try {
      const egg: unknown = JSON.parse(raw);
      if (!egg || typeof egg !== "object" || Array.isArray(egg)) {
        throw new Error("Egg JSON must be an object.");
      }
      await importEgg.mutateAsync(egg);
      e.currentTarget.reset();
    } catch (err) {
      setEggError(err instanceof ApiError || err instanceof Error ? err.message : "Could not import the Egg.");
    }
  }

  if (templates.isError) {
    return <ErrorBanner error={templates.error} fallback="Could not load templates." />;
  }

  return (
    <div className="space-y-6">
      <header>
        <h1 className="text-2xl font-semibold">Templates</h1>
        <p className="text-[var(--text-muted)]">
          Native templates describe the Docker image, ports, and environment for a game. Pterodactyl Eggs are imported
          into that same format — they are not run as-is.
        </p>
      </header>

      {!templates.data ? (
        <LoadingBlock />
      ) : templates.data.length === 0 ? (
        <EmptyState>The catalogue is empty. Create a native template or import an Egg below.</EmptyState>
      ) : (
        <div className="grid gap-3 md:grid-cols-2">
          {templates.data.map((t) => (
            <article
              key={t.id}
              className="rounded-[var(--radius)] border border-[var(--border)] bg-[var(--bg-panel)] px-4 py-3"
            >
              <div className="flex items-baseline justify-between gap-2">
                <h2 className="font-semibold">{t.name}</h2>
                <span className="font-mono text-xs text-[var(--text-faint)]">{t.source}</span>
              </div>
              <p className="mt-1 text-sm text-[var(--text-muted)]">{t.description || "No description."}</p>
              <dl className="mt-3 space-y-1 font-mono text-xs text-[var(--text-muted)]">
                <div>slug {t.slug}</div>
                <div>{t.docker_image}</div>
                <div>{t.memory_mb} MiB</div>
                <div>
                  {(t.ports ?? []).length
                    ? t.ports.map((p) => `${p.name} ${p.protocol}/${p.container_port}`).join(", ")
                    : "no published ports"}
                </div>
              </dl>
            </article>
          ))}
        </div>
      )}

      <div className="grid gap-4 lg:grid-cols-2">
        <Panel title="Create native template">
          <form className="space-y-3" onSubmit={onCreate}>
            <Field id="name" label="Name" required placeholder="Vanilla Minecraft" />
            <Field
              id="slug"
              label="Slug"
              required
              placeholder="vanilla-minecraft"
              hint="Lowercase letters, digits, and hyphens."
            />
            <Field id="description" label="Description" placeholder="Paper on Java 21" />
            <Field id="docker_image" label="Docker image" required placeholder="itzg/minecraft-server:java21" />
            <Field id="memory_mb" label="Memory (MiB)" type="number" min={64} defaultValue={1024} />
            <Field id="startup" label="Startup command (optional)" placeholder="java -jar server.jar nogui" />
            <TextArea
              id="environment"
              label="Environment (optional)"
              hint="JSON object or KEY=value lines."
              placeholder="EULA=true"
            />
            <TextArea
              id="ports"
              label="Ports (optional)"
              hint='One name:protocol:port per line, or a JSON array of { name, protocol, container_port }.'
              placeholder="game:udp:25565"
            />
            {nativeError ? <ErrorBanner error={new Error(nativeError)} fallback={nativeError} /> : null}
            <button type="submit" className={primaryBtn} disabled={create.isPending}>
              {create.isPending ? "Saving…" : "Create template"}
            </button>
          </form>
        </Panel>

        <Panel title="Import Egg">
          <form className="space-y-3" onSubmit={onImport}>
            <TextArea
              id="egg"
              label="Egg JSON"
              required
              hint="Paste a Pterodactyl or Pelican Egg document. It is translated into a native template; unsupported keys are dropped."
              className="min-h-64"
            />
            {eggError ? <ErrorBanner error={new Error(eggError)} fallback={eggError} /> : null}
            <button type="submit" className={primaryBtn} disabled={importEgg.isPending}>
              {importEgg.isPending ? "Importing…" : "Import Egg"}
            </button>
          </form>
        </Panel>
      </div>
    </div>
  );
}
