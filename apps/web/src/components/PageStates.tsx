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
  "inline-flex items-center justify-center gap-2 rounded-[var(--radius)] bg-[var(--accent)] px-4 py-2 font-medium text-[#06221c] shadow-[0_0_0_1px_rgba(62,224,194,0.25)] hover:brightness-110 disabled:opacity-60";
export const secondaryBtn =
  "inline-flex items-center justify-center gap-2 rounded-[var(--radius)] border border-[var(--border)] px-3 py-2 text-sm hover:bg-[var(--bg-hover)] disabled:opacity-60";
export const dangerBtn =
  "inline-flex items-center justify-center gap-2 rounded-[var(--radius)] border border-[var(--danger)]/40 px-3 py-2 text-sm text-[var(--danger)] hover:bg-[var(--danger)]/10 disabled:opacity-60";

export function Panel({
  title,
  children,
  actions,
  className,
}: {
  title?: string;
  children: ReactNode;
  actions?: ReactNode;
  className?: string;
}) {
  return (
    <section
      className={`rounded-[var(--radius)] border border-[var(--border)] bg-[var(--bg-panel)] p-4 shadow-[0_1px_0_rgba(255,255,255,0.03)_inset] ${className ?? ""}`}
    >
      {title || actions ? (
        <div className="mb-3 flex flex-wrap items-center justify-between gap-2">
          {title ? (
            <h2 className="text-sm font-medium uppercase tracking-wide text-[var(--text-faint)]">{title}</h2>
          ) : (
            <span />
          )}
          {actions}
        </div>
      ) : null}
      {children}
    </section>
  );
}

export function PageHeader({
  title,
  description,
  actions,
}: {
  title: string;
  description?: string;
  actions?: ReactNode;
}) {
  return (
    <header className="flex flex-wrap items-end justify-between gap-4">
      <div>
        <h1 className="text-2xl font-semibold tracking-tight">{title}</h1>
        {description ? <p className="mt-1 max-w-2xl text-[var(--text-muted)]">{description}</p> : null}
      </div>
      {actions ? <div className="flex flex-wrap gap-2">{actions}</div> : null}
    </header>
  );
}

export function CopyButton({ text, label = "Copy" }: { text: string; label?: string }) {
  return (
    <button
      type="button"
      className={secondaryBtn}
      onClick={async () => {
        try {
          await navigator.clipboard.writeText(text);
        } catch {
          /* clipboard may be unavailable in some test/dev shells */
        }
      }}
    >
      {label}
    </button>
  );
}
