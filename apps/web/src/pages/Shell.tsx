import { NavLink, Outlet, useLocation, useNavigate } from "react-router-dom";
import { type ReactNode, useEffect, useState } from "react";
import {
  Activity,
  Archive,
  Bell,
  Box,
  ClipboardList,
  LayoutDashboard,
  LayoutTemplate,
  LogOut,
  Menu,
  Server,
  Settings,
  Users,
  X,
} from "lucide-react";
import { useQuery } from "@tanstack/react-query";
import { api, clearSession } from "@fps/api-client";

const NAV = [
  { to: "/", label: "Dashboard", icon: LayoutDashboard },
  { to: "/servers", label: "Servers", icon: Box },
  { to: "/nodes", label: "Nodes", icon: Server },
  { to: "/templates", label: "Templates", icon: LayoutTemplate },
  { to: "/backups", label: "Backups", icon: Archive },
  { to: "/users", label: "Users", icon: Users },
  { to: "/audit", label: "Audit", icon: ClipboardList },
  { to: "/notifications", label: "Notifications", icon: Bell },
  { to: "/settings", label: "Settings", icon: Settings },
] as const;

export function Shell({ version }: { version?: string }) {
  const navigate = useNavigate();
  const location = useLocation();
  const [navOpen, setNavOpen] = useState(false);
  const notes = useQuery({
    queryKey: ["notifications"],
    queryFn: api.notifications,
    refetchInterval: 15_000,
  });
  const unread = notes.data?.filter((n) => !n.read_at).length ?? 0;

  useEffect(() => {
    setNavOpen(false);
  }, [location.pathname]);

  async function logout() {
    try {
      await api.logout();
    } catch {
      /* still clear local session */
    }
    clearSession();
    navigate("/login");
  }

  return (
    <div className="flex min-h-screen">
      {navOpen ? (
        <button
          type="button"
          className="fixed inset-0 z-30 bg-black/50 md:hidden"
          aria-label="Close navigation"
          onClick={() => setNavOpen(false)}
        />
      ) : null}
      <aside
        className={`fixed inset-y-0 left-0 z-40 flex w-60 shrink-0 flex-col border-r border-[var(--border)] bg-[var(--bg-raised)]/90 backdrop-blur-sm transition-transform md:static md:translate-x-0 ${
          navOpen ? "translate-x-0" : "-translate-x-full"
        }`}
      >
        <div className="flex items-start justify-between px-4 py-4">
          <div className="flex items-start gap-3">
            <span
              className="mt-0.5 inline-flex h-9 w-9 items-center justify-center rounded-xl bg-[var(--accent)] font-semibold text-[#06221c]"
              aria-hidden
            >
              F
            </span>
            <div>
              <div className="text-xs uppercase tracking-[0.18em] text-[var(--text-faint)]">Control plane</div>
              <div className="mt-0.5 text-lg font-semibold">FPS</div>
              <div className="font-mono text-xs text-[var(--text-muted)]">{version ?? "…"}</div>
            </div>
          </div>
          <button
            type="button"
            className="rounded p-1 text-[var(--text-muted)] hover:bg-[var(--bg-hover)] md:hidden"
            aria-label="Close navigation"
            onClick={() => setNavOpen(false)}
          >
            <X size={16} />
          </button>
        </div>
        <nav className="flex flex-1 flex-col gap-1 px-2" aria-label="Primary">
          {NAV.map((item) => (
            <Item key={item.to} to={item.to} icon={<item.icon size={16} />} end={item.to === "/"} badge={item.to === "/notifications" ? unread : 0}>
              {item.label}
            </Item>
          ))}
        </nav>
        <button
          type="button"
          onClick={logout}
          className="m-2 flex items-center gap-2 rounded-[var(--radius)] px-3 py-2 text-left text-[var(--text-muted)] hover:bg-[var(--bg-hover)]"
        >
          <LogOut size={16} /> Sign out
        </button>
      </aside>
      <div className="flex min-w-0 flex-1 flex-col">
        <header className="flex items-center justify-between border-b border-[var(--border)] px-4 py-3 md:px-6">
          <div className="flex items-center gap-2 text-sm text-[var(--text-muted)]">
            <button
              type="button"
              className="rounded p-2 hover:bg-[var(--bg-hover)] md:hidden"
              aria-label="Open navigation"
              aria-expanded={navOpen}
              onClick={() => setNavOpen(true)}
            >
              <Menu size={18} />
            </button>
            <Activity size={16} className="text-[var(--ok)]" aria-hidden />
            <span className="hidden sm:inline">Live health from enrolled agents</span>
            <span className="sm:hidden">FPS</span>
          </div>
        </header>
        <main className="flex-1 overflow-auto p-4 md:p-6">
          <Outlet />
        </main>
      </div>
    </div>
  );
}

function Item({
  to,
  icon,
  children,
  end,
  badge,
}: {
  to: string;
  icon: ReactNode;
  children: ReactNode;
  end?: boolean;
  badge?: number;
}) {
  return (
    <NavLink
      to={to}
      end={end}
      className={({ isActive }) =>
        `flex items-center gap-2 rounded-[var(--radius)] px-3 py-2 ${
          isActive
            ? "bg-[var(--accent-dim)] text-[var(--accent)]"
            : "text-[var(--text-muted)] hover:bg-[var(--bg-hover)]"
        }`
      }
    >
      {icon}
      <span className="flex-1">{children}</span>
      {badge ? (
        <span className="rounded-full bg-[var(--accent)] px-1.5 font-mono text-[10px] text-[#06221c]">{badge}</span>
      ) : null}
    </NavLink>
  );
}
