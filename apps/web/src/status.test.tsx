import type { ReactElement } from "react";
import { describe, expect, it } from "vitest";
import { render, screen } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { StatusDot } from "./components/StatusDot";
import { parseEnvironment, parsePorts } from "./components/envFormat";
import { normalizeFiles, statusTone } from "./components/files";
import { consoleSocketUrl } from "@fps/api-client";
import { Shell } from "./pages/Shell";
import { AcceptInvitePage } from "./pages/AcceptInvitePage";

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

describe("statusTone", () => {
  it("maps running servers to the online tone", () => {
    expect(statusTone("running")).toBe("online");
    expect(statusTone("failed")).toBe("offline");
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
