import type { InputHTMLAttributes, ReactNode, SelectHTMLAttributes, TextareaHTMLAttributes } from "react";
import { ApiError } from "@fps/api-client";

export function LoadingBlock() {
  return <div className="h-40 animate-pulse rounded bg-[var(--bg-hover)]" aria-busy="true" />;
}

export function ErrorBanner({ error, fallback }: { error: unknown; fallback: string }) {
  const message = error instanceof ApiError ? error.message : fallback;
  return (
    <p role="alert" className="rounded-[var(--radius)] border border-[var(--danger)]/40 bg-[var(--danger)]/10 px-3 py-2 text-sm">
      {message}
    </p>
  );
}

export function EmptyState({ children }: { children: ReactNode }) {
  return (
    <div className="rounded-[var(--radius)] border border-dashed border-[var(--border-strong)] p-8 text-[var(--text-muted)]">
      {children}
    </div>
  );
}

export function Field({
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

export function TextArea({
  id,
  label,
  hint,
  className,
  ...props
}: TextareaHTMLAttributes<HTMLTextAreaElement> & { id: string; label: string; hint?: string }) {
  return (
    <label className="block">
      <span className="mb-1 block text-sm text-[var(--text-muted)]">{label}</span>
      <textarea
        id={id}
        name={id}
        className={`min-h-24 w-full rounded-[var(--radius)] border border-[var(--border)] bg-[var(--bg)] px-3 py-2 font-mono text-sm ${className ?? ""}`}
        {...props}
      />
      {hint ? <span className="mt-1 block text-xs text-[var(--text-faint)]">{hint}</span> : null}
    </label>
  );
}

export function Select({
  id,
  label,
  hint,
  children,
  ...props
}: SelectHTMLAttributes<HTMLSelectElement> & { id: string; label: string; hint?: string; children: ReactNode }) {
  return (
    <label className="block">
      <span className="mb-1 block text-sm text-[var(--text-muted)]">{label}</span>
      <select
        id={id}
        name={id}
        className="w-full rounded-[var(--radius)] border border-[var(--border)] bg-[var(--bg)] px-3 py-2"
        {...props}
      >
        {children}
      </select>
      {hint ? <span className="mt-1 block text-xs text-[var(--text-faint)]">{hint}</span> : null}
    </label>
  );
}

export const primaryBtn =
  "rounded-[var(--radius)] bg-[var(--accent)] px-4 py-2 font-medium text-[#06221c] disabled:opacity-60";
export const secondaryBtn =
  "rounded-[var(--radius)] border border-[var(--border)] px-3 py-2 text-sm hover:bg-[var(--bg-hover)] disabled:opacity-60";
export const dangerBtn =
  "rounded-[var(--radius)] border border-[var(--danger)]/40 px-3 py-2 text-sm text-[var(--danger)] hover:bg-[var(--danger)]/10 disabled:opacity-60";

export function Panel({ title, children }: { title?: string; children: ReactNode }) {
  return (
    <section className="rounded-[var(--radius)] border border-[var(--border)] bg-[var(--bg-panel)] p-4">
      {title ? <h2 className="mb-3 text-sm font-medium uppercase tracking-wide text-[var(--text-faint)]">{title}</h2> : null}
      {children}
    </section>
  );
}
