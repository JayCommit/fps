export type Problem = {
  type: string;
  title: string;
  status: number;
  detail: string;
  field?: string;
};

export class ApiError extends Error {
  status: number;
  problem: Problem | null;

  constructor(status: number, problem: Problem | null, fallback: string) {
    super(problem?.detail ?? fallback);
    this.status = status;
    this.problem = problem;
  }
}

export type SetupStatus = {
  completed: boolean;
  product: string;
  version: string;
};

export type User = {
  id: string;
  email: string;
  display_name: string;
  role: string;
  totp_enabled: boolean;
  status: string;
  created_at: string;
};

export type SessionResponse = {
  user: User;
  permissions: string[];
  access_token: string;
  refresh_token: string;
  csrf_token: string;
  expires_in: number;
  mfa_required: boolean;
};

export type NodeHealth = {
  id: string;
  status: "enrolling" | "online" | "degraded" | "offline" | "maintenance";
  docker: "available" | "unavailable" | "error";
  last_heartbeat_at: string | null;
  agent_version: string | null;
  protocol_version: number;
  resources: {
    cpu_cores?: number;
    memory_bytes?: number;
    disk_bytes?: number;
    disk_available_bytes?: number;
  };
  message: string;
};

export type NodeView = {
  id: string;
  name: string;
  hostname: string;
  architecture: string | null;
  operating_system: string | null;
  enrolled_at: string;
  workload_count: number;
  health: NodeHealth;
};

export type DashboardSummary = {
  product: string;
  version: string;
  setup_completed: boolean;
  nodes_total: number;
  nodes_online: number;
  nodes_degraded: number;
  nodes_offline: number;
  docker_available: number;
  alerts: { severity: string; title: string; detail: string }[];
  servers_total?: number;
  servers_running?: number;
};

export type PortMapping = {
  name: string;
  protocol: string;
  container_port: number;
};

export type TemplateSummary = {
  id: string;
  name: string;
  slug: string;
  description: string;
  docker_image: string;
  memory_mb: number;
  source: string;
  ports: PortMapping[];
  startup_command?: string | null;
  cpu_shares?: number;
  volume_path?: string;
  created_at?: string;
};

export type NativeTemplateInput = {
  name: string;
  slug: string;
  description: string;
  docker_image: string;
  environment?: Record<string, string>;
  ports?: PortMapping[];
  memory_mb?: number;
  startup?: string;
};

export type ServerSummary = {
  id: string;
  name: string;
  template_id: string;
  node_id: string | null;
  status: string;
  memory_mb: number;
  last_error: string | null;
  container_name: string | null;
  created_at: string;
  allocation_id?: string | null;
  cpu_shares?: number;
  updated_at?: string;
  environment?: Record<string, string>;
};

export type ServerLogChunk = {
  stream: string;
  chunk: string;
  created_at: string;
};

export type BackupSummary = {
  id: string;
  server_id: string;
  node_id: string;
  status: string;
  archive_path: string | null;
  size_bytes: number | null;
  error: string | null;
  created_at: string;
  completed_at: string | null;
};

export type ScheduleSummary = {
  id: string;
  server_id: string;
  name: string;
  interval_seconds: number;
  action: string;
  enabled: boolean;
  last_run_at?: string | null;
  next_run_at?: string | null;
  created_at?: string;
};

export type Invitation = {
  id: string;
  email: string;
  role: string;
  expires_at: string;
  accepted_at: string | null;
};

export type InvitationCreated = {
  id: string;
  email: string;
  role: string;
  token: string;
  expires_at: string;
};

export type AuditEvent = {
  id: string;
  action: string;
  resource_type: string;
  resource_id: string | null;
  actor_user_id: string | null;
  created_at: string;
  details: unknown;
};

export type NotificationItem = {
  id: string;
  kind: string;
  title: string;
  body: string;
  read_at: string | null;
  created_at: string;
};

export type NodeRevokeResponse = {
  id: string;
  revoked: true;
};

export type VersionInfo = {
  name: string;
  package: string;
  version: string;
  api: string;
  node_protocol: number;
  database_schema: number;
  channel: string;
};

const TOKEN_KEY = "fps.access_token";
const REFRESH_KEY = "fps.refresh_token";

export function getToken(): string | null {
  return localStorage.getItem(TOKEN_KEY);
}

export function setSession(session: SessionResponse) {
  localStorage.setItem(TOKEN_KEY, session.access_token);
  if (session.refresh_token) {
    localStorage.setItem(REFRESH_KEY, session.refresh_token);
  }
}

export function clearSession() {
  localStorage.removeItem(TOKEN_KEY);
  localStorage.removeItem(REFRESH_KEY);
}

let refreshInFlight: Promise<boolean> | null = null;

async function tryRefresh(): Promise<boolean> {
  const refresh = localStorage.getItem(REFRESH_KEY);
  if (!refresh) {
    return false;
  }
  if (!refreshInFlight) {
    refreshInFlight = (async () => {
      try {
        const res = await fetch("/v1/auth/refresh", {
          method: "POST",
          headers: { accept: "application/json", "content-type": "application/json" },
          body: JSON.stringify({ refresh_token: refresh }),
        });
        if (!res.ok) {
          return false;
        }
        const session = (await res.json()) as SessionResponse;
        setSession(session);
        return true;
      } catch {
        return false;
      }
    })().finally(() => {
      refreshInFlight = null;
    });
  }
  return refreshInFlight;
}

function shouldAttemptRefresh(path: string): boolean {
  return (
    !path.startsWith("/v1/auth/login") &&
    !path.startsWith("/v1/auth/refresh") &&
    !path.startsWith("/v1/setup") &&
    !path.startsWith("/v1/invitations/accept")
  );
}

async function request<T>(path: string, init: RequestInit = {}, retried = false): Promise<T> {
  const headers = new Headers(init.headers);
  headers.set("accept", "application/json");
  if (init.body && !headers.has("content-type")) {
    headers.set("content-type", "application/json");
  }
  const token = getToken();
  if (token) headers.set("authorization", `Bearer ${token}`);
  const res = await fetch(path, { ...init, headers });
  if (res.status === 204) return undefined as T;
  const text = await res.text();
  const data = text ? JSON.parse(text) : null;
  if (!res.ok) {
    if (res.status === 401 && !retried && shouldAttemptRefresh(path)) {
      const refreshed = await tryRefresh();
      if (refreshed) {
        return request<T>(path, init, true);
      }
      clearSession();
      if (typeof window !== "undefined" && window.location.pathname !== "/login") {
        window.location.href = "/login";
      }
    }
    throw new ApiError(res.status, data, res.statusText);
  }
  return data as T;
}

export const api = {
  health: () => fetch("/health").then((r) => r.ok),
  version: () => request<VersionInfo>("/version"),
  setupStatus: () => request<SetupStatus>("/v1/setup/status"),
  setup: (body: { email: string; password: string; display_name: string }) =>
    request<SessionResponse>("/v1/setup", { method: "POST", body: JSON.stringify(body) }),
  login: (body: { email: string; password: string; totp_code?: string; recovery_code?: string }) =>
    request<SessionResponse>("/v1/auth/login", { method: "POST", body: JSON.stringify(body) }),
  me: () => request<{ user: User; permissions: string[] }>("/v1/auth/me"),
  logout: () => request<void>("/v1/auth/logout", { method: "POST" }),
  dashboard: () => request<DashboardSummary>("/v1/dashboard"),
  nodes: () => request<NodeView[]>("/v1/nodes"),
  node: (id: string) => request<NodeView>(`/v1/nodes/${id}`),
  createEnrollmentToken: (label?: string) =>
    request<{ token: string; expires_at: string }>("/v1/nodes/enrollment-tokens", {
      method: "POST",
      body: JSON.stringify({ label }),
    }),
  revokeNode: (id: string) =>
    request<NodeRevokeResponse>(`/v1/nodes/${id}/revoke`, { method: "POST" }),
  users: () => request<User[]>("/v1/users"),
  updateUser: (id: string, body: { role?: string; status?: "active" | "disabled" }) =>
    request<User>(`/v1/users/${id}`, { method: "PATCH", body: JSON.stringify(body) }),
  invitations: () => request<Invitation[]>("/v1/invitations"),
  createInvitation: (body: { email: string; role: string }) =>
    request<InvitationCreated>("/v1/invitations", { method: "POST", body: JSON.stringify(body) }),
  acceptInvitation: (body: { token: string; password: string; display_name: string }) =>
    request<SessionResponse>("/v1/invitations/accept", { method: "POST", body: JSON.stringify(body) }),
  audit: () => request<AuditEvent[]>("/v1/audit"),
  notifications: () => request<NotificationItem[]>("/v1/notifications"),
  markNotificationRead: (id: string) =>
    request<void>(`/v1/notifications/${id}/read`, { method: "POST" }),
  templates: () => request<TemplateSummary[]>("/v1/templates"),
  createTemplate: (body: NativeTemplateInput) =>
    request<TemplateSummary>("/v1/templates", { method: "POST", body: JSON.stringify(body) }),
  importEgg: (egg: unknown) =>
    request<TemplateSummary>("/v1/templates/import-egg", { method: "POST", body: JSON.stringify(egg) }),
  servers: () => request<ServerSummary[]>("/v1/servers"),
  createServer: (body: { name: string; template_id: string; environment?: Record<string, string> }) =>
    request<ServerSummary>("/v1/servers", { method: "POST", body: JSON.stringify(body) }),
  server: (id: string) => request<ServerSummary>(`/v1/servers/${id}`),
  serverStart: (id: string) => request<unknown>(`/v1/servers/${id}/start`, { method: "POST" }),
  serverStop: (id: string) => request<unknown>(`/v1/servers/${id}/stop`, { method: "POST" }),
  serverBackup: (id: string) => request<unknown>(`/v1/servers/${id}/backup`, { method: "POST" }),
  serverLogs: (id: string) => request<ServerLogChunk[]>(`/v1/servers/${id}/logs`),
  serverFiles: (id: string) => request<unknown>(`/v1/servers/${id}/files`),
  refreshServerFiles: (id: string) =>
    request<void>(`/v1/servers/${id}/files/refresh`, { method: "POST" }),
  backups: (serverId?: string) =>
    request<BackupSummary[]>(
      `/v1/backups${serverId ? `?server_id=${encodeURIComponent(serverId)}` : ""}`,
    ),
  schedules: (serverId?: string) =>
    request<ScheduleSummary[]>(
      `/v1/schedules${serverId ? `?server_id=${encodeURIComponent(serverId)}` : ""}`,
    ),
  createSchedule: (body: {
    server_id: string;
    name: string;
    interval_seconds: number;
    action: "start" | "stop" | "backup";
  }) => request<ScheduleSummary>("/v1/schedules", { method: "POST", body: JSON.stringify(body) }),
  updateSchedule: (id: string, body: { enabled?: boolean }) =>
    request<ScheduleSummary>(`/v1/schedules/${id}`, { method: "PATCH", body: JSON.stringify(body) }),
};
