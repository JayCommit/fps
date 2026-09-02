import { type FormEvent, useState } from "react";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import { Link, useNavigate } from "react-router-dom";
import { api, ApiError } from "@fps/api-client";
import { GAMES } from "../components/GameIcon";
import { EnvEditor, envToRows, rowsToEnv } from "../components/EnvEditor";
import {
  ErrorBanner,
  Field,
  PageHeader,
  Panel,
  Select,
  TextArea,
  primaryBtn,
  secondaryBtn,
} from "../components/PageStates";
import { parsePorts } from "../components/envFormat";

export function CreateTemplatePage() {
  const qc = useQueryClient();
  const navigate = useNavigate();
  const [nativeError, setNativeError] = useState<string | null>(null);
  const [envRows, setEnvRows] = useState(envToRows({}));

  const create = useMutation({
    mutationFn: api.createTemplate,
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["templates"] });
      navigate("/templates");
    },
  });

  async function onCreate(e: FormEvent<HTMLFormElement>) {
    e.preventDefault();
    setNativeError(null);
    const form = new FormData(e.currentTarget);
    try {
      await create.mutateAsync({
        name: String(form.get("name") ?? "").trim(),
        slug: String(form.get("slug") ?? "").trim(),
        game: String(form.get("game") ?? "").trim() || undefined,
        description: String(form.get("description") ?? "").trim(),
        docker_image: String(form.get("docker_image") ?? "").trim(),
        memory_mb: Number(form.get("memory_mb") || 64),
        startup: String(form.get("startup") ?? "").trim() || undefined,
        environment: rowsToEnv(envRows),
        ports: parsePorts(String(form.get("ports") ?? "")),
      });
    } catch (err) {
      setNativeError(err instanceof ApiError || err instanceof Error ? err.message : "Could not create the template.");
    }
  }

  return (
    <div className="space-y-6">
      <PageHeader
        title="Create template"
        description="A native template is a Docker image plus ports, memory, and environment. The agent never runs Egg install scripts."
        actions={
          <Link to="/templates" className={secondaryBtn}>
            Back to catalogue
          </Link>
        }
      />
      <Panel>
        <form className="grid gap-4 lg:grid-cols-2" onSubmit={onCreate}>
          <Field id="name" label="Name" required placeholder="Vanilla Minecraft" />
          <Field
            id="slug"
            label="Slug"
            required
            placeholder="vanilla-minecraft"
            hint="Lowercase letters, digits, and hyphens."
          />
          <Select id="game" label="Game icon" defaultValue="custom">
            {GAMES.map((g) => (
              <option key={g.key} value={g.key}>
                {g.label}
              </option>
            ))}
          </Select>
          <Field id="docker_image" label="Docker image" required placeholder="itzg/minecraft-server:java21" />
          <Field id="description" label="Description" placeholder="Paper on Java 21" />
          <Field id="memory_mb" label="Memory (MiB)" type="number" min={64} defaultValue={1024} />
          <div className="lg:col-span-2">
            <Field id="startup" label="Startup command (optional)" placeholder="java -jar server.jar nogui" />
          </div>
          <div className="lg:col-span-2">
            <EnvEditor
              id="environment"
              label="Environment"
              hint="One variable per row. Values can be overridden when you deploy a server."
              rows={envRows}
              onChange={setEnvRows}
            />
          </div>
          <div className="lg:col-span-2">
            <TextArea
              id="ports"
              label="Ports (optional)"
              hint="One name:protocol:port per line, or a JSON array of { name, protocol, container_port }."
              placeholder={"game:tcp:25565\nquery:udp:25565"}
            />
          </div>
          {nativeError ? (
            <div className="lg:col-span-2">
              <ErrorBanner error={new Error(nativeError)} fallback={nativeError} />
            </div>
          ) : null}
          <div className="lg:col-span-2">
            <button type="submit" className={primaryBtn} disabled={create.isPending}>
              {create.isPending ? "Saving…" : "Create template"}
            </button>
          </div>
        </form>
      </Panel>
    </div>
  );
}

export function ImportEggPage() {
  const qc = useQueryClient();
  const navigate = useNavigate();
  const [eggError, setEggError] = useState<string | null>(null);
  const importEgg = useMutation({
    mutationFn: api.importEgg,
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["templates"] });
      navigate("/templates");
    },
  });

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
    } catch (err) {
      setEggError(err instanceof ApiError || err instanceof Error ? err.message : "Could not import the Egg.");
    }
  }

  return (
    <div className="space-y-6">
      <PageHeader
        title="Import Egg"
        description="Paste a Pterodactyl or Pelican Egg. Unsupported keys are dropped and the result is stored as a native template."
        actions={
          <Link to="/templates" className={secondaryBtn}>
            Back to catalogue
          </Link>
        }
      />
      <Panel>
        <form className="space-y-3" onSubmit={onImport}>
          <TextArea
            id="egg"
            label="Egg JSON"
            required
            hint="The control plane never executes Egg install scripts on the host."
            className="min-h-80"
          />
          {eggError ? <ErrorBanner error={new Error(eggError)} fallback={eggError} /> : null}
          <button type="submit" className={primaryBtn} disabled={importEgg.isPending}>
            {importEgg.isPending ? "Importing…" : "Import Egg"}
          </button>
        </form>
      </Panel>
    </div>
  );
}
