export type AllocatedPortView = {
  name: string;
  protocol: string;
  container_port: number;
  host_port: number;
  ip: string;
};

/** Format a published mapping as `ip:host/protocol → container (name)`. */
export function formatAllocatedPort(port: AllocatedPortView): string {
  const ip = port.ip || "0.0.0.0";
  return `${ip}:${port.host_port}/${port.protocol} → ${port.container_port} (${port.name})`;
}

export function primaryAllocatedPort(ports?: AllocatedPortView[] | null): AllocatedPortView | undefined {
  if (!ports?.length) return undefined;
  return ports.find((p) => p.name === "game") ?? ports[0];
}
