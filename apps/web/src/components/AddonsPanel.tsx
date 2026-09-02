import { Puzzle } from "lucide-react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { api, ApiError, type ServerAddonView } from "@fps/api-client";
import { StatusDot } from "./StatusDot";
import { EmptyState, ErrorBanner, LoadingBlock, Panel, dangerBtn, primaryBtn, secondaryBtn } from "./PageStates";

function addonTone(status: string): "online" | "degraded" | "offline" | "unknown" {
  if (status === "installed") return "online";
  if (status === "queued" || status === "uninstalling") return "degraded";
  if (status === "failed") return "offline";
  return "unknown";
}

function statusLabel(status: string): string {
  switch (status) {
    case "installed":
      return "Installed";
    case "queued":
      return "Installing…";
    case "uninstalling":
      return "Removing…";
    case "failed":
      return "Failed";
    default:
      return "Available";
  }
}

function categoryLabel(category: string): string {
  switch (category) {
    case "loader":
      return "Loader";
    case "framework":
      return "Framework";
    case "plugin":
      return "Plugin";
    case "resource":
      return "Resource";
    default:
      return category;
  }
}

export function groupAddons(addons: ServerAddonView[]): { category: string; items: ServerAddonView[] }[] {
  const order = ["loader", "framework", "plugin", "resource"];
  const groups = new Map<string, ServerAddonView[]>();
  for (const addon of addons) {
    const key = addon.category || "plugin";
    const list = groups.get(key) ?? [];
    list.push(addon);
    groups.set(key, list);
  }
  const keys = [...groups.keys()].sort((a, b) => {
    const ia = order.indexOf(a);
    const ib = order.indexOf(b);
    return (ia === -1 ? 99 : ia) - (ib === -1 ? 99 : ib);
  });
  return keys.map((category) => ({ category, items: groups.get(category) ?? [] }));
}

export function AddonsPanel({ serverId }: { serverId: string }) {
  const qc = useQueryClient();
  const addons = useQuery({
    queryKey: ["server-addons", serverId],
    queryFn: () => api.serverAddons(serverId),
    refetchInterval: 5_000,
  });
  const install = useMutation({
    mutationFn: (slug: string) => api.installAddon(serverId, slug),
    onSuccess: () => qc.invalidateQueries({ queryKey: ["server-addons", serverId] }),
  });
  const uninstall = useMutation({
    mutationFn: (slug: string) => api.uninstallAddon(serverId, slug),
    onSuccess: () => qc.invalidateQueries({ queryKey: ["server-addons", serverId] }),
  });

  return (
    <Panel
      title="Addons"
      actions={
        <span className="inline-flex items-center gap-1 text-xs text-[var(--text-faint)]">
          <Puzzle size={14} aria-hidden />
          Install loaders and plugins into this server volume
        </span>
      }
    >
      {addons.isError ? (
        <ErrorBanner error={addons.error} fallback="Could not load addons for this game." />
      ) : !addons.data ? (
        <LoadingBlock />
      ) : addons.data.length === 0 ? (
        <EmptyState>
          No curated addons for this game yet. Minecraft Paper, CS2, Rust, FiveM, and Garry&apos;s Mod have one-click
          loaders and plugins here.
        </EmptyState>
      ) : (
        <div className="space-y-5">
          {groupAddons(addons.data).map((group) => (
            <div key={group.category}>
              <h3 className="mb-2 text-xs font-medium uppercase tracking-wide text-[var(--text-faint)]">
                {categoryLabel(group.category)}
              </h3>
              <ul className="space-y-2">
                {group.items.map((addon) => (
                  <AddonRow
                    key={addon.slug}
                    addon={addon}
                    busy={install.isPending || uninstall.isPending}
                    onInstall={() => install.mutate(addon.slug)}
                    onUninstall={() => uninstall.mutate(addon.slug)}
                    actionError={
                      (install.variables === addon.slug && install.error) ||
                      (uninstall.variables === addon.slug && uninstall.error) ||
                      null
                    }
                  />
                ))}
              </ul>
            </div>
          ))}
        </div>
      )}
    </Panel>
  );
}

function AddonRow({
  addon,
  busy,
  onInstall,
  onUninstall,
  actionError,
}: {
  addon: ServerAddonView;
  busy: boolean;
  onInstall: () => void;
  onUninstall: () => void;
  actionError: unknown;
}) {
  const pending = addon.status === "queued" || addon.status === "uninstalling";
  const installed = addon.status === "installed";
  const failed = addon.status === "failed";
  return (
    <li className="rounded-[var(--radius)] border border-[var(--border)] px-3 py-3">
      <div className="flex flex-wrap items-start justify-between gap-3">
        <div className="min-w-0 flex-1">
          <div className="flex flex-wrap items-center gap-2">
            <StatusDot status={addonTone(addon.status)} />
            <span className="font-medium">{addon.name}</span>
            <span className="text-xs text-[var(--text-muted)]">{statusLabel(addon.status)}</span>
            {addon.restart_required ? (
              <span className="rounded-full border border-[var(--border)] px-2 py-0.5 text-[10px] uppercase tracking-wide text-[var(--text-faint)]">
                Restart
              </span>
            ) : null}
          </div>
          <p className="mt-1 text-sm text-[var(--text-muted)]">{addon.description}</p>
          {addon.depends_on.length ? (
            <p className="mt-1 text-xs text-[var(--text-faint)]">Requires {addon.depends_on.join(", ")}</p>
          ) : null}
          {addon.notes ? <p className="mt-1 text-xs text-[var(--text-faint)]">{addon.notes}</p> : null}
          {addon.error ? <p className="mt-1 text-sm text-[var(--danger)]">{addon.error}</p> : null}
          {actionError ? (
            <p className="mt-1 text-sm text-[var(--danger)]">
              {actionError instanceof ApiError ? actionError.message : "Could not update this addon."}
            </p>
          ) : null}
        </div>
        <div className="flex shrink-0 gap-2">
          {installed || failed ? (
            <button type="button" className={dangerBtn} disabled={busy || pending} onClick={onUninstall}>
              Uninstall
            </button>
          ) : null}
          {!installed || failed ? (
            <button
              type="button"
              className={installed || failed ? secondaryBtn : primaryBtn}
              disabled={busy || pending}
              onClick={onInstall}
            >
              {failed ? "Retry" : pending ? "Working…" : "Install"}
            </button>
          ) : null}
        </div>
      </div>
    </li>
  );
}

export { statusLabel, addonTone };
