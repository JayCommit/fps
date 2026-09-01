import { type FormEvent, useState } from "react";
import { useSearchParams } from "react-router-dom";
import { useQueryClient } from "@tanstack/react-query";
import { api, ApiError, setSession } from "@fps/api-client";
import { ErrorBanner, Field, primaryBtn } from "../components/PageStates";

export function AcceptInvitePage() {
  const qc = useQueryClient();
  const [params] = useSearchParams();
  const tokenFromQuery = params.get("token") ?? "";
  const [error, setError] = useState<string | null>(null);
  const [pending, setPending] = useState(false);

  async function onSubmit(e: FormEvent<HTMLFormElement>) {
    e.preventDefault();
    setError(null);
    setPending(true);
    const form = new FormData(e.currentTarget);
    try {
      const session = await api.acceptInvitation({
        token: String(form.get("token") ?? "").trim(),
        display_name: String(form.get("display_name") ?? "").trim(),
        password: String(form.get("password") ?? ""),
      });
      setSession(session);
      await qc.invalidateQueries();
      window.location.href = "/";
    } catch (err) {
      setError(err instanceof ApiError ? err.message : "Could not accept the invitation.");
    } finally {
      setPending(false);
    }
  }

  return (
    <main className="flex min-h-screen items-center justify-center p-6">
      <section className="w-full max-w-md rounded-[var(--radius)] border border-[var(--border)] bg-[var(--bg-panel)] p-6 shadow-[var(--shadow)]">
        <p className="text-xs uppercase tracking-[0.18em] text-[var(--text-faint)]">FPS</p>
        <h1 className="mt-2 text-2xl font-semibold">Accept invitation</h1>
        <p className="mt-2 text-sm text-[var(--text-muted)]">
          Create your operator account with the one-time token from your invite. This page does not require a session.
        </p>
        <form className="mt-6 space-y-4" onSubmit={onSubmit}>
          <Field
            id="token"
            label="Invite token"
            required
            defaultValue={tokenFromQuery}
            autoComplete="off"
            hint={tokenFromQuery ? "Filled from the invite link." : "Paste the token from the invitation."}
          />
          <Field id="display_name" label="Display name" autoComplete="name" required />
          <Field
            id="password"
            label="Password"
            type="password"
            autoComplete="new-password"
            minLength={12}
            hint="At least 12 characters. Stored with Argon2id."
            required
          />
          {error ? <ErrorBanner error={new Error(error)} fallback={error} /> : null}
          <button type="submit" disabled={pending} className={`w-full ${primaryBtn}`}>
            {pending ? "Creating account…" : "Accept and sign in"}
          </button>
        </form>
      </section>
    </main>
  );
}
