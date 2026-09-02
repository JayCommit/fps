import { type FormEvent, useEffect, useRef, useState } from "react";
import { api, consoleSocketUrl, getToken } from "@fps/api-client";
import { EmptyState, Field, primaryBtn } from "../components/PageStates";

type Line = { stream: string; chunk: string; created_at?: string };

export function LiveConsole({ serverId }: { serverId: string }) {
  const [lines, setLines] = useState<Line[]>([]);
  const [connected, setConnected] = useState(false);
  const [command, setCommand] = useState("");
  const scroller = useRef<HTMLPreElement>(null);

  useEffect(() => {
    const token = getToken();
    if (!token) return;
    const url = consoleSocketUrl(serverId, token);
    const ws = new WebSocket(url);
    ws.onopen = () => setConnected(true);
    ws.onclose = () => setConnected(false);
    ws.onmessage = (ev) => {
      try {
        const msg = JSON.parse(String(ev.data)) as { type?: string; stream?: string; chunk?: string; created_at?: string };
        if (msg.type === "log" && msg.chunk) {
          setLines((prev) => [...prev.slice(-400), { stream: msg.stream ?? "stdout", chunk: msg.chunk!, created_at: msg.created_at }]);
        }
      } catch {
        /* ignore */
      }
    };
    return () => ws.close();
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
        {connected ? "Live WebSocket console" : "Connecting… logs fall back to HTTP poll on the Logs panel if the socket is blocked."}
      </p>
      {lines.length === 0 ? (
        <EmptyState>Waiting for container output.</EmptyState>
      ) : (
        <pre
          ref={scroller}
          className="max-h-80 overflow-auto rounded-[var(--radius)] bg-[var(--bg)] p-3 font-mono text-xs leading-5"
        >
          <code>
            {lines
              .map((line) => `${line.stream}: ${line.chunk.replace(/\n$/, "")}`)
              .join("\n")}
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
          />
        </div>
        <div className="flex items-end">
          <button type="submit" className={primaryBtn}>
            Send
          </button>
        </div>
      </form>
    </div>
  );
}
