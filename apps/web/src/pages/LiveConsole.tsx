import { type FormEvent, useEffect, useRef, useState } from "react";
import { api, consoleSocketUrl, getToken } from "@fps/api-client";
import { EmptyState, Field, primaryBtn } from "../components/PageStates";

type Line = { stream: string; chunk: string; created_at?: string };

export function LiveConsole({ serverId, status }: { serverId: string; status?: string }) {
  const [lines, setLines] = useState<Line[]>([]);
  const [connected, setConnected] = useState(false);
  const [command, setCommand] = useState("");
  const scroller = useRef<HTMLPreElement>(null);
  const stdinDisabled = status === "installing" || status === "deleting";
  const watchingInstall = status === "installing" || status === "deleting";

  useEffect(() => {
    const token = getToken();
    if (!token) return;
    const url = consoleSocketUrl(serverId, token);
    const ws = new WebSocket(url);
    ws.onopen = () => setConnected(true);
    ws.onclose = () => setConnected(false);
    ws.onerror = () => setConnected(false);
    ws.onmessage = (ev) => {
      try {
        const msg = JSON.parse(String(ev.data)) as {
          type?: string;
          stream?: string;
          chunk?: string;
          created_at?: string;
        };
        if (msg.type === "log" && msg.chunk) {
          setLines((prev) => [
            ...prev.slice(-400),
            { stream: msg.stream ?? "stdout", chunk: msg.chunk!, created_at: msg.created_at },
          ]);
        }
      } catch {
        /* ignore */
      }
    };
    return () => {
      setConnected(false);
      ws.close();
    };
  }, [serverId]);

  useEffect(() => {
    if (connected) return;
    let cancelled = false;
    async function poll() {
      try {
        const chunks = await api.serverLogs(serverId);
        if (!cancelled) {
          setLines(
            chunks.slice(-400).map((line) => ({
              stream: line.stream,
              chunk: line.chunk,
              created_at: line.created_at,
            })),
          );
        }
      } catch {
        /* HTTP poll is a fallback; the Logs panel still works. */
      }
    }
    void poll();
    const timer = window.setInterval(() => void poll(), 3_000);
    return () => {
      cancelled = true;
      window.clearInterval(timer);
    };
  }, [connected, serverId]);

  useEffect(() => {
    scroller.current?.scrollTo(0, scroller.current.scrollHeight);
  }, [lines]);

  async function onExec(e: FormEvent) {
    e.preventDefault();
    if (stdinDisabled) return;
    const cmd = command.trim();
    if (!cmd) return;
    setCommand("");
    try {
      await api.execServer(serverId, cmd);
    } catch {
      /* heartbeat will surface job errors */
    }
  }

  return (
    <div>
      <p className="mb-2 text-xs text-[var(--text-muted)]">
        {connected
          ? watchingInstall
            ? "Live WebSocket console — watching pull and install output."
            : "Live WebSocket console"
          : "Connecting… logs fall back to HTTP poll if the socket is blocked."}
      </p>
      {lines.length === 0 ? (
        <EmptyState>
          {status === "installing"
            ? "Watching install: image pull and container start will appear here."
            : status === "deleting"
              ? "Watching delete: container teardown output will appear here."
              : "Waiting for container output."}
        </EmptyState>
      ) : (
        <pre
          ref={scroller}
          className="max-h-80 overflow-auto rounded-[var(--radius)] bg-[var(--bg)] p-3 font-mono text-xs leading-5"
        >
          <code>
            {splitConsoleLines(lines).map((row, i) => (
              <span
                key={i}
                className={
                  row.stream === "install"
                    ? "text-[var(--accent)]"
                    : row.stream === "stderr"
                      ? "text-[var(--danger)]"
                      : undefined
                }
              >
                {row.stream}: {row.text}
                {"\n"}
              </span>
            ))}
          </code>
        </pre>
      )}
      <form className="mt-3 flex gap-2" onSubmit={onExec}>
        <div className="flex-1">
          <Field
            id="console_cmd"
            label="Command"
            value={command}
            onChange={(e) => setCommand(e.target.value)}
            placeholder="say hello"
            disabled={stdinDisabled}
          />
        </div>
        <div className="flex items-end">
          <button type="submit" className={primaryBtn} disabled={stdinDisabled}>
            Send
          </button>
        </div>
      </form>
    </div>
  );
}

export function splitConsoleLines(lines: Line[]): { stream: string; text: string }[] {
  const out: { stream: string; text: string }[] = [];
  for (const line of lines) {
    const parts = line.chunk.replace(/\n$/, "").split("\n");
    for (const text of parts) {
      out.push({ stream: line.stream, text });
    }
  }
  return out;
}
