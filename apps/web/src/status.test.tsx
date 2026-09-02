import type { ReactElement } from "react";
import { describe, expect, it } from "vitest";
import { render, screen } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { StatusDot } from "./components/StatusDot";
import { parseEnvironment, parsePorts } from "./components/envFormat";
import { rowsToEnv } from "./components/EnvEditor";
import { inferGameKey } from "./components/GameIcon";
import { groupAddons } from "./components/AddonsPanel";
import { formatUptime, normalizeFiles, statusTone, usagePercent } from "./components/files";
import { consoleSocketUrl } from "@fps/api-client";
import { Shell } from "./pages/Shell";
import { AcceptInvitePage } from "./pages/AcceptInvitePage";
import { formatAllocatedPort } from "./components/ports";
import { splitConsoleLines } from "./pages/LiveConsole";

describe("StatusDot", () => {
  it("renders without accessible name noise", () => {
    const { container } = render(<StatusDot status="online" />);
    expect(container.querySelector("[aria-hidden]")).toBeTruthy();
  });
});

describe("parseEnvironment", () => {
  it("reads KEY=value lines", () => {
    expect(parseEnvironment("EULA=true\nDIFFICULTY=hard")).toEqual({
      EULA: "true",
      DIFFICULTY: "hard",
    });
  });

  it("reads a JSON object", () => {
    expect(parseEnvironment('{"EULA":"true"}')).toEqual({ EULA: "true" });
  });

  it("returns undefined for blank input", () => {
    expect(parseEnvironment("  \n  ")).toBeUndefined();
  });
});

describe("parsePorts", () => {
  it("reads name:protocol:port lines", () => {
    expect(parsePorts("game:udp:25565")).toEqual([
      { name: "game", protocol: "udp", container_port: 25565 },
    ]);
  });
});

describe("normalizeFiles", () => {
  it("accepts a bare array or a files wrapper", () => {
    expect(normalizeFiles(["world", "server.jar"]).map((f) => f.name)).toEqual(["world", "server.jar"]);
    expect(normalizeFiles({ files: [{ name: "eula.txt", size: 12 }] })).toEqual([
      { name: "eula.txt", path: undefined, size: 12, is_dir: false, modified_at: undefined },
    ]);
  });
});

describe("inferGameKey", () => {
  it("maps popular slugs onto icon keys", () => {
    expect(inferGameKey("fivem-txadmin", "FiveM (txAdmin)")).toBe("fivem");
    expect(inferGameKey("cs2", "Counter-Strike 2")).toBe("cs2");
    expect(inferGameKey("rust", "Rust")).toBe("rust");
    expect(inferGameKey("minecraft-paper", "Paper")).toBe("minecraft");
  });
});

describe("rowsToEnv", () => {
  it("drops blank keys", () => {
    expect(rowsToEnv([{ key: "  ", value: "x" }, { key: "FOO", value: "bar" }])).toEqual({
      FOO: "bar",
    });
  });
});

describe("statusTone", () => {
  it("maps running servers to the online tone", () => {
    expect(statusTone("running")).toBe("online");
    expect(statusTone("failed")).toBe("offline");
    expect(statusTone("maintenance")).toBe("degraded");
  });
});

describe("host usage helpers", () => {
  it("formats uptime and usage percent for the node panel", () => {
    expect(formatUptime(3661)).toBe("1h 1m");
    expect(formatUptime(90_000)).toBe("1d 1h");
    expect(usagePercent(5, 10)).toBe(50);
    expect(usagePercent(null, 10)).toBeNull();
  });
});

describe("groupAddons", () => {
  it("orders loaders before plugins", () => {
    const groups = groupAddons([
      {
        slug: "mc-luckperms",
        name: "LuckPerms",
        description: "perms",
        category: "plugin",
        version_label: "latest",
        depends_on: [],
        restart_required: true,
        notes: "",
        status: "available",
      },
      {
        slug: "cs2-metamod",
        name: "MetaMod",
        description: "loader",
        category: "loader",
        version_label: "latest",
        depends_on: [],
        restart_required: true,
        notes: "",
        status: "installed",
      },
    ]);
    expect(groups.map((g) => g.category)).toEqual(["loader", "plugin"]);
    expect(groups[0].items[0].slug).toBe("cs2-metamod");
  });
});

function wrap(ui: ReactElement) {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return (
    <QueryClientProvider client={client}>
      <MemoryRouter>{ui}</MemoryRouter>
    </QueryClientProvider>
  );
}

describe("Shell", () => {
  it("lists the product navigation items", () => {
    render(wrap(<Shell version="0.0.1-alpha.1" />));
    const nav = screen.getByRole("navigation", { name: "Primary" });
    for (const label of ["Dashboard", "Servers", "Nodes", "Templates", "Backups", "Users", "Audit", "Notifications", "Settings"]) {
      expect(nav.textContent).toContain(label);
    }
    expect(screen.getByRole("button", { name: "Open navigation" })).toBeTruthy();
  });
});

describe("consoleSocketUrl", () => {
  it("turns the HTTP API origin into a websocket console URL", () => {
    expect(consoleSocketUrl("abc", "tok")).toContain("/v1/servers/abc/console?access_token=tok");
  });
});

describe("formatAllocatedPort", () => {
  it("formats host:port/protocol to container name", () => {
    expect(
      formatAllocatedPort({
        host_port: 25565,
        protocol: "tcp",
        container_port: 25565,
        name: "game",
        ip: "0.0.0.0",
      }),
    ).toBe("0.0.0.0:25565/tcp → 25565 (game)");
  });
});

describe("splitConsoleLines", () => {
  it("splits chunks that contain newlines", () => {
    expect(
      splitConsoleLines([
        { stream: "install", chunk: "pulling\nstarting\n" },
        { stream: "stdout", chunk: "hello" },
      ]),
    ).toEqual([
      { stream: "install", text: "pulling" },
      { stream: "install", text: "starting" },
      { stream: "stdout", text: "hello" },
    ]);
  });
});

describe("AcceptInvitePage", () => {
  it("renders the public accept form", () => {
    render(
      <QueryClientProvider client={new QueryClient({ defaultOptions: { queries: { retry: false } } })}>
        <MemoryRouter initialEntries={["/invite?token=abc"]}>
          <AcceptInvitePage />
        </MemoryRouter>
      </QueryClientProvider>,
    );
    expect(screen.getByLabelText(/Invite token/)).toBeTruthy();
    expect(screen.getByRole("button", { name: "Accept and sign in" })).toBeTruthy();
  });
});
