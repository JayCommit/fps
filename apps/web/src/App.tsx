import { type FormEvent, useState } from "react";
import { Navigate, Route, Routes } from "react-router-dom";
import { useQuery } from "@tanstack/react-query";
import { api, getApiBase, getToken, setApiBase } from "@fps/api-client";
import { LoginPage } from "./pages/LoginPage";
import { SetupPage } from "./pages/SetupPage";
import { Shell } from "./pages/Shell";
import { DashboardPage } from "./pages/DashboardPage";
import { NodesPage } from "./pages/NodesPage";
import { NodeDetailPage } from "./pages/NodeDetailPage";
import { ServersPage } from "./pages/ServersPage";
import { ServerDetailPage } from "./pages/ServerDetailPage";
import { TemplatesPage } from "./pages/TemplatesPage";
import { UsersPage } from "./pages/UsersPage";
import { AuditPage } from "./pages/AuditPage";
import { BackupsPage } from "./pages/BackupsPage";
import { NotificationsPage } from "./pages/NotificationsPage";
import { AcceptInvitePage } from "./pages/AcceptInvitePage";
import { SettingsPage } from "./pages/SettingsPage";
import { OfflineBanner } from "./components/OfflineBanner";

export function App() {
  const setup = useQuery({ queryKey: ["setup"], queryFn: api.setupStatus });
  const version = useQuery({ queryKey: ["version"], queryFn: api.version });

  if (setup.isError) {
    return <ConnectControlPlane />;
  }

  if (setup.isLoading || !setup.data) {
    return (
      <main className="p-8" aria-busy="true">
        <div className="h-8 w-48 animate-pulse rounded bg-[var(--bg-hover)]" />
        <div className="mt-4 h-32 animate-pulse rounded bg-[var(--bg-hover)]" />
      </main>
    );
  }

  return (
    <>
      <OfflineBanner />
      <Routes>
        {!setup.data.completed ? (
          <Route path="*" element={<SetupPage product={setup.data.product} />} />
        ) : (
          <>
            <Route path="/login" element={<LoginPage />} />
            <Route path="/invite" element={<AcceptInvitePage />} />
            <Route
              path="/"
              element={
                getToken() ? (
                  <Shell version={version.data?.version} />
                ) : (
                  <Navigate to="/login" replace />
                )
              }
            >
              <Route index element={<DashboardPage />} />
              <Route path="servers" element={<ServersPage />} />
              <Route path="servers/:id" element={<ServerDetailPage />} />
              <Route path="nodes" element={<NodesPage />} />
              <Route path="nodes/:id" element={<NodeDetailPage />} />
              <Route path="templates" element={<TemplatesPage />} />
              <Route path="backups" element={<BackupsPage />} />
              <Route path="users" element={<UsersPage />} />
              <Route path="audit" element={<AuditPage />} />
              <Route path="notifications" element={<NotificationsPage />} />
              <Route path="settings" element={<SettingsPage />} />
            </Route>
            <Route path="*" element={<Navigate to="/" replace />} />
          </>
        )}
      </Routes>
    </>
  );
}

function ConnectControlPlane() {
  const [url, setUrl] = useState(getApiBase() || "http://127.0.0.1:47890");

  function onSubmit(e: FormEvent<HTMLFormElement>) {
    e.preventDefault();
    setApiBase(url);
    window.location.reload();
  }

  return (
    <main className="mx-auto max-w-lg p-8">
      <h1 className="text-xl font-semibold">Control plane unreachable</h1>
      <p className="mt-2 text-[var(--text-muted)]">
        The UI could not reach the API. Start <code>fps-control-plane</code> on port 47890, or
        enter the URL of a running control plane (desktop companion and remote installs).
      </p>
      <form className="mt-6 space-y-3" onSubmit={onSubmit}>
        <label className="block">
          <span className="mb-1 block text-sm text-[var(--text-muted)]">Control plane URL</span>
          <input
            value={url}
            onChange={(e) => setUrl(e.target.value)}
            className="w-full rounded-[var(--radius)] border border-[var(--border)] bg-[var(--bg)] px-3 py-2"
            placeholder="http://PANEL_IP:47890"
            autoComplete="url"
          />
        </label>
        <button
          type="submit"
          className="rounded-[var(--radius)] bg-[var(--accent)] px-4 py-2 font-medium text-[#06221c]"
        >
          Save and retry
        </button>
      </form>
    </main>
  );
}
