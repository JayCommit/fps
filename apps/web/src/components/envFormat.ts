/** Parse a JSON object or KEY=value lines into environment variables. */
export function parseEnvironment(text: string): Record<string, string> | undefined {
  const trimmed = text.trim();
  if (!trimmed) return undefined;
  if (trimmed.startsWith("{")) {
    const parsed: unknown = JSON.parse(trimmed);
    if (!parsed || typeof parsed !== "object" || Array.isArray(parsed)) {
      throw new Error("Environment JSON must be an object of string values.");
    }
    const out: Record<string, string> = {};
    for (const [key, value] of Object.entries(parsed as Record<string, unknown>)) {
      out[key] = typeof value === "string" ? value : JSON.stringify(value);
    }
    return out;
  }
  const out: Record<string, string> = {};
  for (const raw of trimmed.split("\n")) {
    const line = raw.trim();
    if (!line || line.startsWith("#")) continue;
    const eq = line.indexOf("=");
    if (eq <= 0) {
      throw new Error(`Invalid environment line "${line}". Use KEY=value or a JSON object.`);
    }
    out[line.slice(0, eq).trim()] = line.slice(eq + 1);
  }
  return Object.keys(out).length ? out : undefined;
}

export function parsePorts(text: string): { name: string; protocol: string; container_port: number }[] | undefined {
  const trimmed = text.trim();
  if (!trimmed) return undefined;
  if (trimmed.startsWith("[")) {
    const parsed: unknown = JSON.parse(trimmed);
    if (!Array.isArray(parsed)) {
      throw new Error("Ports JSON must be an array.");
    }
    return parsed.map((item) => {
      const row = item as { name?: string; protocol?: string; container_port?: number };
      return {
        name: String(row.name ?? "game"),
        protocol: String(row.protocol ?? "tcp"),
        container_port: Number(row.container_port ?? 0),
      };
    });
  }
  const ports = [];
  for (const raw of trimmed.split("\n")) {
    const line = raw.trim();
    if (!line) continue;
    const [name, protocol, port] = line.split(":");
    if (!name || !protocol || !port) {
      throw new Error(`Invalid port line "${line}". Use name:protocol:port or a JSON array.`);
    }
    ports.push({ name, protocol, container_port: Number(port) });
  }
  return ports.length ? ports : undefined;
}
