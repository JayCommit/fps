import { type FormEvent, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { api, ApiError } from "@fps/api-client";
import {
  dangerBtn,
  EmptyState,
  ErrorBanner,
  Field,
  LoadingBlock,
  Panel,
  primaryBtn,
  Select,
} from "../components/PageStates";
import { formatWhen, statusTone } from "../components/files";
import { StatusDot } from "../components/StatusDot";

const ROLES = ["administrator", "operator", "viewer"] as const;

export function UsersPage() {
  const qc = useQueryClient();
  const me = useQuery({ queryKey: ["me"], queryFn: api.me });
  const users = useQuery({ queryKey: ["users"], queryFn: api.users });
  const invitations = useQuery({ queryKey: ["invitations"], queryFn: api.invitations });
  const [inviteError, setInviteError] = useState<string | null>(null);
  const [issuedToken, setIssuedToken] = useState<string | null>(null);

  const invite = useMutation({
    mutationFn: api.createInvitation,
    onSuccess: (res) => {
      setIssuedToken(res.token);
      qc.invalidateQueries({ queryKey: ["invitations"] });
    },
  });
  const patchUser = useMutation({
    mutationFn: ({ id, status }: { id: string; status: "active" | "disabled" }) => api.updateUser(id, { status }),
    onSuccess: () => qc.invalidateQueries({ queryKey: ["users"] }),
  });

  async function onInvite(e: FormEvent<HTMLFormElement>) {
    e.preventDefault();
    setInviteError(null);
    setIssuedToken(null);
    const form = new FormData(e.currentTarget);
    const email = String(form.get("email") ?? "").trim();
    const role = String(form.get("role") ?? "viewer");
    try {
      await invite.mutateAsync({ email, role });
      e.currentTarget.reset();
    } catch (err) {
      setInviteError(err instanceof ApiError ? err.message : "Could not send the invitation.");
    }
  }

  if (users.isError) {
    return <ErrorBanner error={users.error} fallback="Could not load users." />;
  }

  const selfId = me.data?.user.id;

  return (
    <div className="space-y-6">
      <header>
        <h1 className="text-2xl font-semibold">Users</h1>
        <p className="text-[var(--text-muted)]">
          Invitation-only. New operators redeem a one-time token from the invite email or the accept page.
        </p>
      </header>

      <Panel title="Invite">
        <form className="grid gap-3 sm:grid-cols-3" onSubmit={onInvite}>
          <Field id="email" label="Email" type="email" required autoComplete="off" />
          <Select id="role" label="Role" defaultValue="operator">
            {ROLES.map((role) => (
              <option key={role} value={role}>
                {role}
              </option>
            ))}
          </Select>
          <div className="flex items-end">
            <button type="submit" className={primaryBtn} disabled={invite.isPending}>
              {invite.isPending ? "Inviting…" : "Send invitation"}
            </button>
          </div>
        </form>
        {inviteError ? <div className="mt-3"><ErrorBanner error={new Error(inviteError)} fallback={inviteError} /></div> : null}
        {issuedToken ? (
          <div className="mt-3 rounded-[var(--radius)] border border-[var(--accent)]/40 bg-[var(--accent-dim)] p-3">
            <p className="text-sm">One-time invite token (shown once). Share the accept link; the token cannot be retrieved again.</p>
            <code className="mt-2 block break-all font-mono text-sm">{issuedToken}</code>
            <p className="mt-2 font-mono text-xs text-[var(--text-muted)]">
              {`${window.location.origin}/invite?token=${issuedToken}`}
            </p>
          </div>
        ) : null}
      </Panel>

      {!users.data ? (
        <LoadingBlock />
      ) : users.data.length === 0 ? (
        <EmptyState>No users returned from the control plane.</EmptyState>
      ) : (
        <div className="overflow-x-auto rounded-[var(--radius)] border border-[var(--border)]">
          <table className="w-full text-left text-sm">
            <thead className="bg-[var(--bg-raised)] text-xs uppercase tracking-wide text-[var(--text-faint)]">
              <tr>
                <th className="px-4 py-2">User</th>
                <th className="px-4 py-2">Role</th>
                <th className="px-4 py-2">Status</th>
                <th className="px-4 py-2">MFA</th>
                <th className="px-4 py-2" />
              </tr>
            </thead>
            <tbody>
              {users.data.map((u) => {
                const isSelf = u.id === selfId;
                return (
                  <tr key={u.id} className="border-t border-[var(--border)]">
                    <td className="px-4 py-3">
                      <div className="font-medium">{u.display_name}</div>
                      <div className="text-xs text-[var(--text-muted)]">{u.email}</div>
                    </td>
                    <td className="px-4 py-3">{u.role}</td>
                    <td className="px-4 py-3">
                      <span className="inline-flex items-center gap-2">
                        <StatusDot status={statusTone(u.status)} />
                        {u.status}
                      </span>
                    </td>
                    <td className="px-4 py-3 font-mono text-xs">{u.totp_enabled ? "totp" : "off"}</td>
                    <td className="px-4 py-3 text-right">
                      {isSelf ? (
                        <span className="text-xs text-[var(--text-faint)]">you</span>
                      ) : u.status === "disabled" ? (
                        <button
                          type="button"
                          className={primaryBtn}
                          disabled={patchUser.isPending}
                          onClick={() => patchUser.mutate({ id: u.id, status: "active" })}
                        >
                          Enable
                        </button>
                      ) : (
                        <button
                          type="button"
                          className={dangerBtn}
                          disabled={patchUser.isPending}
                          onClick={() => patchUser.mutate({ id: u.id, status: "disabled" })}
                        >
                          Disable
                        </button>
                      )}
                    </td>
                  </tr>
                );
              })}
            </tbody>
          </table>
        </div>
      )}

      <Panel title="Pending invitations">
        {invitations.isError ? (
          <ErrorBanner error={invitations.error} fallback="Could not load invitations." />
        ) : !invitations.data ? (
          <LoadingBlock />
        ) : invitations.data.length === 0 ? (
          <p className="text-sm text-[var(--text-muted)]">No invitations issued.</p>
        ) : (
          <ul className="space-y-2 text-sm">
            {invitations.data.map((inv) => (
              <li key={inv.id} className="flex flex-wrap items-center justify-between gap-2">
                <span>
                  {inv.email} · {inv.role}
                  {inv.accepted_at ? (
                    <span className="ml-2 text-xs text-[var(--ok)]">accepted {formatWhen(inv.accepted_at)}</span>
                  ) : (
                    <span className="ml-2 text-xs text-[var(--text-muted)]">expires {formatWhen(inv.expires_at)}</span>
                  )}
                </span>
              </li>
            ))}
          </ul>
        )}
      </Panel>
    </div>
  );
}
