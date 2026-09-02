import { type FormEvent, type InputHTMLAttributes, type ReactNode, useState } from "react";
import { api, ApiError, setApiBase, setSession } from "@fps/api-client";
import { useQueryClient } from "@tanstack/react-query";

export function SetupPage({ product }: { product: string }) {
  const qc = useQueryClient();
  const [error, setError] = useState<string | null>(null);
  const [pending, setPending] = useState(false);

  async function onSubmit(e: FormEvent<HTMLFormElement>) {
    e.preventDefault();
    setError(null);
    setPending(true);
    const form = new FormData(e.currentTarget);
    try {
      const session = await api.setup({
        email: String(form.get("email")),
        password: String(form.get("password")),
        display_name: String(form.get("display_name")),
      });
      setSession(session);
      await qc.invalidateQueries();
    } catch (err) {
      setError(err instanceof ApiError ? err.message : "Setup failed.");
    } finally {
      setPending(false);
    }
  }

  return (
    <AuthCard
      title={`Create the ${product} owner`}
      subtitle="Invitation-only alpha. The first account is the platform owner and cannot be created twice."
    >
      <form className="space-y-4" onSubmit={onSubmit}>
        <Field id="display_name" label="Display name" autoComplete="name" required />
        <Field id="email" label="Email" type="email" autoComplete="username" required />
        <Field
          id="password"
          label="Password"
          type="password"
          autoComplete="new-password"
          minLength={12}
          hint="At least 12 characters. Stored with Argon2id."
          required
        />
        {error ? <ErrorText>{error}</ErrorText> : null}
        <button
          type="submit"
          disabled={pending}
          className="w-full rounded-[var(--radius)] bg-[var(--accent)] px-4 py-2 font-medium text-[#06221c] disabled:opacity-60"
        >
          {pending ? "Creating owner…" : "Create owner"}
        </button>
      </form>
    </AuthCard>
  );
}

export function LoginPage() {
  const qc = useQueryClient();
  const [error, setError] = useState<string | null>(null);
  const [pending, setPending] = useState(false);
  const [mfa, setMfa] = useState(false);

  async function onSubmit(e: FormEvent<HTMLFormElement>) {
    e.preventDefault();
    setError(null);
    setPending(true);
    const form = new FormData(e.currentTarget);
    const apiBase = String(form.get("api_base") || "").trim();
    if (apiBase) setApiBase(apiBase);
    try {
      const session = await api.login({
        email: String(form.get("email")),
        password: String(form.get("password")),
        totp_code: mfa ? String(form.get("totp_code") || "") : undefined,
        recovery_code: mfa ? String(form.get("recovery_code") || "") || undefined : undefined,
      });
      setSession(session);
      await qc.invalidateQueries();
      window.location.href = "/";
    } catch (err) {
      if (err instanceof ApiError && err.problem?.type?.includes("mfa_required")) {
        setMfa(true);
        setError("Enter the six-digit authenticator code.");
      } else {
        setError(err instanceof ApiError ? err.message : "Sign-in failed.");
      }
    } finally {
      setPending(false);
    }
  }

  return (
    <AuthCard title="Sign in" subtitle="Use the owner account created during setup.">
      <form className="space-y-4" onSubmit={onSubmit}>
        <Field id="email" label="Email" type="email" autoComplete="username" required />
        <Field id="password" label="Password" type="password" autoComplete="current-password" required />
        <Field
          id="api_base"
          label="Control plane URL (desktop / remote)"
          placeholder="http://127.0.0.1:47890"
          hint="Leave blank when this page is served by the control plane."
        />
        {mfa ? (
          <>
            <Field id="totp_code" label="Authenticator code" inputMode="numeric" autoComplete="one-time-code" />
            <Field id="recovery_code" label="Recovery code (optional)" autoComplete="off" />
          </>
        ) : null}
        {error ? <ErrorText>{error}</ErrorText> : null}
        <button
          type="submit"
          disabled={pending}
          className="w-full rounded-[var(--radius)] bg-[var(--accent)] px-4 py-2 font-medium text-[#06221c] disabled:opacity-60"
        >
          {pending ? "Signing in…" : "Sign in"}
        </button>
      </form>
    </AuthCard>
  );
}

function AuthCard({
  title,
  subtitle,
  children,
}: {
  title: string;
  subtitle: string;
  children: ReactNode;
}) {
  return (
    <main className="flex min-h-screen items-center justify-center p-6">
      <section className="w-full max-w-md rounded-[var(--radius)] border border-[var(--border)] bg-[var(--bg-panel)] p-6 shadow-[var(--shadow)]">
        <div className="mb-4 inline-flex h-10 w-10 items-center justify-center rounded-xl bg-[var(--accent)] font-semibold text-[#06221c]">
          F
        </div>
        <p className="text-xs uppercase tracking-[0.18em] text-[var(--text-faint)]">FPS</p>
        <h1 className="mt-2 text-2xl font-semibold">{title}</h1>
        <p className="mt-2 text-sm text-[var(--text-muted)]">{subtitle}</p>
        <div className="mt-6">{children}</div>
      </section>
    </main>
  );
}

function Field({
  id,
  label,
  hint,
  ...props
}: InputHTMLAttributes<HTMLInputElement> & { id: string; label: string; hint?: string }) {
  return (
    <label className="block">
      <span className="mb-1 block text-sm text-[var(--text-muted)]">{label}</span>
      <input
        id={id}
        name={id}
        className="w-full rounded-[var(--radius)] border border-[var(--border)] bg-[var(--bg)] px-3 py-2"
        {...props}
      />
      {hint ? <span className="mt-1 block text-xs text-[var(--text-faint)]">{hint}</span> : null}
    </label>
  );
}

function ErrorText({ children }: { children: ReactNode }) {
  return (
    <p role="alert" className="rounded-[var(--radius)] border border-[var(--danger)]/40 bg-[var(--danger)]/10 px-3 py-2 text-sm">
      {children}
    </p>
  );
}
