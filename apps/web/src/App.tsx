import { Navigate, Route, Routes } from "react-router-dom";
import { useQuery } from "@tanstack/react-query";
import { api, getToken } from "@fps/api-client";
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
import { OfflineBanner } from "./components/OfflineBanner";

export function App() {
  const setup = useQuery({ queryKey: ["setup"], queryFn: api.setupStatus });
  const version = useQuery({ queryKey: ["version"], queryFn: api.version });

  if (setup.isError) {
    return (
      <main className="mx-auto max-w-lg p-8">
        <h1 className="text-xl font-semibold">Control plane unreachable</h1>
        <p className="mt-2 text-[var(--text-muted)]">
          The web UI could not reach the API. Start the control plane on port 47890
          and reload.
        </p>
      </main>
    );
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
            </Route>
            <Route path="*" element={<Navigate to="/" replace />} />
          </>
        )}
      </Routes>
    </>
  );
}
