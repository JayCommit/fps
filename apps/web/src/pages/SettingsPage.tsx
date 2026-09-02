import { type FormEvent, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { api, ApiError, getApiBase, setApiBase } from "@fps/api-client";
import { EmptyState, ErrorBanner, Field, LoadingBlock, Panel, primaryBtn, TextArea } from "../components/PageStates";

export function SettingsPage() {
  const qc = useQueryClient();
  const settings = useQuery({ queryKey: ["settings"], queryFn: api.settings });
  const me = useQuery({ queryKey: ["me"], queryFn: api.me });
  const updates = useQuery({ queryKey: ["updates"], queryFn: api.checkUpdates });
  const [notesError, setNotesError] = useState<string | null>(null);
  const [totpUrl, setTotpUrl] = useState<string | null>(null);
  const [recovery, setRecovery] = useState<string[] | null>(null);
  const [totpError, setTotpError] = useState<string | null>(null);
  const [apiUrl, setApiUrl] = useState(getApiBase());

  const saveNotes = useMutation({
    mutationFn: (operator_notes: string) => api.patchSettings({ operator_notes }),
    onSuccess: () => qc.invalidateQueries({ queryKey: ["settings"] }),
  });
  const startTotp = useMutation({ mutationFn: api.totpStart });
  const confirmTotp = useMutation({ mutationFn: (code: string) => api.totpConfirm(code) });

  async function onNotes(e: FormEvent<HTMLFormElement>) {
    e.preventDefault();
    setNotesError(null);
    const form = new FormData(e.currentTarget);
    try {
      await saveNotes.mutateAsync(String(form.get("operator_notes") ?? ""));
    } catch (err) {
      setNotesError(err instanceof ApiError ? err.message : "Could not save notes.");
    }
  }

  async function onTotpStart() {
    setTotpError(null);
    try {
      const res = await startTotp.mutateAsync();
      setTotpUrl(res.otpauth_url);
    } catch (err) {
      setTotpError(err instanceof ApiError ? err.message : "Could not start TOTP.");
    }
  }

  async function onTotpConfirm(e: FormEvent<HTMLFormElement>) {
    e.preventDefault();
    setTotpError(null);
    const form = new FormData(e.currentTarget);
    try {
      const res = await confirmTotp.mutateAsync(String(form.get("code") ?? ""));
      setRecovery(res.recovery_codes);
      setTotpUrl(null);
      await qc.invalidateQueries({ queryKey: ["me"] });
    } catch (err) {
      setTotpError(err instanceof ApiError ? err.message : "That code was not accepted.");
    }
  }

  function onSaveApi(e: FormEvent<HTMLFormElement>) {
    e.preventDefault();
    setApiBase(apiUrl);
    window.location.reload();
  }

  if (settings.isError) {
    return <ErrorBanner error={settings.error} fallback="Could not load settings." />;
  }
  if (!settings.data) {
    return <LoadingBlock />;
  }

  return (
    <div className="space-y-6">
      <header>
        <h1 className="text-2xl font-semibold">Settings</h1>
        <p className="text-[var(--text-muted)]">
          Control plane {settings.data.product} {settings.data.version}
        </p>
      </header>

      <Panel title="Connection">
        <dl className="mb-4 grid gap-2 text-sm sm:grid-cols-2">
          <div>
            <dt className="text-[var(--text-faint)]">Public URL</dt>
            <dd className="font-mono">{settings.data.public_url}</dd>
          </div>
          <div>
            <dt className="text-[var(--text-faint)]">Insecure HTTP</dt>
            <dd>{settings.data.allow_insecure_http ? "allowed (alpha)" : "refused"}</dd>
          </div>
        </dl>
        <form className="grid gap-3 sm:grid-cols-[1fr_auto]" onSubmit={onSaveApi}>
          <Field
            id="api_base"
            label="Companion / desktop API URL"
            value={apiUrl}
            onChange={(e) => setApiUrl(e.target.value)}
            placeholder="http://PANEL_IP:47890"
            hint="Leave blank to use this origin (typical when the panel serves the UI)."
          />
          <div className="flex items-end">
            <button type="submit" className={primaryBtn}>
              Save URL
            </button>
          </div>
        </form>
      </Panel>

      <Panel title="Updates">
        {updates.isError ? (
          <ErrorBanner error={updates.error} fallback="Could not check updates." />
        ) : !updates.data ? (
          <LoadingBlock />
        ) : (
          <div className="space-y-2 text-sm">
            <p>
              Channel <span className="font-mono">{updates.data.channel}</span> · running{" "}
              <span className="font-mono">{updates.data.current_version}</span>
            </p>
            <p className="text-[var(--text-muted)]">{updates.data.message}</p>
            <a className="text-[var(--accent)] underline" href={updates.data.releases_url}>
              GitHub Releases
            </a>
          </div>
        )}
      </Panel>

      <Panel title="Authenticator (TOTP)">
        <p className="mb-3 text-sm text-[var(--text-muted)]">
          {me.data?.user.totp_enabled
            ? "TOTP is enabled on this account."
            : "Optional second factor. Scan the otpauth URL in your authenticator app, then confirm."}
        </p>
        {totpError ? <ErrorBanner error={new Error(totpError)} fallback={totpError} /> : null}
        {recovery ? (
          <div>
            <p className="mb-2 text-sm">Store these recovery codes now. They are shown once.</p>
            <ul className="font-mono text-sm">
              {recovery.map((c) => (
                <li key={c}>{c}</li>
              ))}
            </ul>
          </div>
        ) : totpUrl ? (
          <form className="space-y-3" onSubmit={onTotpConfirm}>
            <p className="break-all font-mono text-xs">{totpUrl}</p>
            <Field id="code" label="Six-digit code" inputMode="numeric" autoComplete="one-time-code" required />
            <button type="submit" className={primaryBtn} disabled={confirmTotp.isPending}>
              {confirmTotp.isPending ? "Confirming…" : "Confirm TOTP"}
            </button>
          </form>
        ) : (
          <button type="button" className={primaryBtn} onClick={onTotpStart} disabled={startTotp.isPending}>
            {startTotp.isPending ? "Starting…" : "Enable TOTP"}
          </button>
        )}
      </Panel>

      <Panel title="Operator notes">
        {notesError ? <ErrorBanner error={new Error(notesError)} fallback={notesError} /> : null}
        <form className="space-y-3" onSubmit={onNotes}>
          <TextArea
            id="operator_notes"
            label="Notes"
            defaultValue={settings.data.operator_notes ?? ""}
            hint="Visible to administrators. Stored in platform settings."
          />
          <button type="submit" className={primaryBtn} disabled={saveNotes.isPending}>
            {saveNotes.isPending ? "Saving…" : "Save notes"}
          </button>
        </form>
      </Panel>

      {!me.data ? <EmptyState>Sign in to manage MFA.</EmptyState> : null}
    </div>
  );
}
