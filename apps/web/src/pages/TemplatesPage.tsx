import { useMemo, useState } from "react";
import { Link } from "react-router-dom";
import { useQuery } from "@tanstack/react-query";
import { Plus, Upload } from "lucide-react";
import { api } from "@fps/api-client";
import { GameIcon, GAMES, inferGameKey } from "../components/GameIcon";
import {
  EmptyState,
  ErrorBanner,
  LoadingBlock,
  PageHeader,
  primaryBtn,
  secondaryBtn,
} from "../components/PageStates";

export function TemplatesPage() {
  const templates = useQuery({ queryKey: ["templates"], queryFn: api.templates });
  const [filter, setFilter] = useState("all");
  const [q, setQ] = useState("");

  const filtered = useMemo(() => {
    const list = templates.data ?? [];
    return list.filter((t) => {
      const game = inferGameKey(t.slug, t.name, t.game);
      if (filter !== "all" && game !== filter) return false;
      if (!q.trim()) return true;
      const hay = `${t.name} ${t.slug} ${t.description} ${t.docker_image}`.toLowerCase();
      return hay.includes(q.trim().toLowerCase());
    });
  }, [templates.data, filter, q]);

  if (templates.isError) {
    return <ErrorBanner error={templates.error} fallback="Could not load templates." />;
  }

  return (
    <div className="space-y-6">
      <PageHeader
        title="Templates"
        description="Native Docker recipes for popular games. Eggs import into this same format — they are never run as host scripts."
        actions={
          <>
            <Link to="/templates/new" className={primaryBtn}>
              <Plus size={16} /> Create template
            </Link>
            <Link to="/templates/import" className={secondaryBtn}>
              <Upload size={16} /> Import Egg
            </Link>
          </>
        }
      />

      <div className="flex flex-wrap items-center gap-2">
        <input
          value={q}
          onChange={(e) => setQ(e.target.value)}
          placeholder="Search templates"
          className="min-w-56 flex-1 rounded-[var(--radius)] border border-[var(--border)] bg-[var(--bg)] px-3 py-2 sm:max-w-xs"
        />
        <div className="flex flex-wrap gap-1">
          <FilterChip active={filter === "all"} onClick={() => setFilter("all")}>
            All
          </FilterChip>
          {GAMES.filter((g) => g.key !== "custom").map((g) => (
            <FilterChip key={g.key} active={filter === g.key} onClick={() => setFilter(g.key)}>
              {g.label}
            </FilterChip>
          ))}
        </div>
      </div>

      {!templates.data ? (
        <LoadingBlock />
      ) : templates.data.length === 0 ? (
        <EmptyState>
          The catalogue is empty.{" "}
          <Link className="text-[var(--accent)]" to="/templates/new">
            Create a native template
          </Link>
          .
        </EmptyState>
      ) : filtered.length === 0 ? (
        <EmptyState>No templates match that filter.</EmptyState>
      ) : (
        <div className="grid gap-3 sm:grid-cols-2 xl:grid-cols-3">
          {filtered.map((t) => {
            const ports = t.ports ?? [];
            return (
              <article
                key={t.id}
                className="ui-card overflow-hidden rounded-[var(--radius)] border border-[var(--border)] bg-[var(--bg-panel)]"
              >
                <div className="flex items-start gap-3 p-4">
                  <GameIcon slug={t.slug} name={t.name} game={t.game} size="lg" />
                  <div className="min-w-0 flex-1">
                    <div className="flex items-start justify-between gap-2">
                      <h2 className="font-semibold">{t.name}</h2>
                      <span className="rounded-full border border-[var(--border)] px-2 py-0.5 font-mono text-[10px] uppercase tracking-wide text-[var(--text-faint)]">
                        {t.source}
                      </span>
                    </div>
                    <p className="mt-1 line-clamp-2 text-sm text-[var(--text-muted)]">
                      {t.description || "No description."}
                    </p>
                  </div>
                </div>
                <dl className="grid grid-cols-2 gap-px border-t border-[var(--border)] bg-[var(--border)] font-mono text-[11px]">
                  <div className="bg-[var(--bg-panel)] px-4 py-2">
                    <dt className="text-[var(--text-faint)]">Memory</dt>
                    <dd>{t.memory_mb} MiB</dd>
                  </div>
                  <div className="bg-[var(--bg-panel)] px-4 py-2">
                    <dt className="text-[var(--text-faint)]">Ports</dt>
                    <dd>
                      {ports.length
                        ? ports.map((p) => `${p.protocol}/${p.container_port}`).join(", ")
                        : "none"}
                    </dd>
                  </div>
                  <div className="col-span-2 bg-[var(--bg-panel)] px-4 py-2">
                    <dt className="text-[var(--text-faint)]">Image</dt>
                    <dd className="truncate text-[var(--text-muted)]">{t.docker_image}</dd>
                  </div>
                </dl>
                <div className="flex items-center justify-between px-4 py-3">
                  <span className="font-mono text-xs text-[var(--text-faint)]">{t.slug}</span>
                  <Link to={`/servers/new`} className="text-sm text-[var(--accent)]">
                    Deploy
                  </Link>
                </div>
              </article>
            );
          })}
        </div>
      )}
    </div>
  );
}

function FilterChip({
  active,
  onClick,
  children,
}: {
  active: boolean;
  onClick: () => void;
  children: string;
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      className={`rounded-full px-2.5 py-1 text-xs ${
        active
          ? "bg-[var(--accent-dim)] text-[var(--accent)]"
          : "text-[var(--text-muted)] hover:bg-[var(--bg-hover)]"
      }`}
    >
      {children}
    </button>
  );
}
