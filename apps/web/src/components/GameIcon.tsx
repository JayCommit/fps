import type { LucideIcon } from "lucide-react";
import {
  AudioLines,
  Box,
  CarFront,
  Cog,
  Crosshair,
  Factory,
  Flame,
  Headphones,
  Layers,
  PawPrint,
  Shield,
  Swords,
  Wrench,
} from "lucide-react";

export type GameMeta = {
  key: string;
  label: string;
  from: string;
  to: string;
  Icon: LucideIcon;
};

export const GAMES: GameMeta[] = [
  { key: "minecraft", label: "Minecraft", from: "#3d8c40", to: "#1d4d20", Icon: Box },
  { key: "fivem", label: "FiveM", from: "#f59e0b", to: "#9a3412", Icon: CarFront },
  { key: "cs2", label: "Counter-Strike 2", from: "#fb923c", to: "#7c2d12", Icon: Crosshair },
  { key: "rust", label: "Rust", from: "#ef4444", to: "#7f1d1d", Icon: Flame },
  { key: "valheim", label: "Valheim", from: "#60a5fa", to: "#1e3a8a", Icon: Shield },
  { key: "palworld", label: "Palworld", from: "#fbbf24", to: "#b45309", Icon: PawPrint },
  { key: "factorio", label: "Factorio", from: "#f97316", to: "#9a3412", Icon: Cog },
  { key: "terraria", label: "Terraria", from: "#a78bfa", to: "#5b21b6", Icon: Swords },
  { key: "gmod", label: "Garry's Mod", from: "#38bdf8", to: "#0f766e", Icon: Wrench },
  { key: "teamspeak", label: "TeamSpeak", from: "#818cf8", to: "#312e81", Icon: Headphones },
  { key: "satisfactory", label: "Satisfactory", from: "#facc15", to: "#854d0e", Icon: Factory },
  { key: "demo", label: "Demo", from: "#2dd4bf", to: "#115e59", Icon: AudioLines },
  { key: "custom", label: "Custom", from: "#64748b", to: "#1e293b", Icon: Layers },
];

const BY_KEY = new Map(GAMES.map((g) => [g.key, g]));

export function inferGameKey(slug?: string, name?: string, game?: string | null): string {
  const explicit = (game ?? "").trim().toLowerCase();
  if (explicit && BY_KEY.has(explicit)) return explicit;
  const hay = `${slug ?? ""} ${name ?? ""}`.toLowerCase();
  for (const g of GAMES) {
    if (g.key === "custom" || g.key === "demo") continue;
    if (hay.includes(g.key)) return g.key;
  }
  if (hay.includes("txadmin") || hay.includes("citizenfx") || hay.includes("gta")) return "fivem";
  if (hay.includes("counter-strike") || hay.includes("csgo")) return "cs2";
  if (hay.includes("paper") || hay.includes("spigot") || hay.includes("bedrock")) return "minecraft";
  if (hay.includes("garry")) return "gmod";
  if (hay.includes("ts3")) return "teamspeak";
  if (hay.includes("http-echo") || hay.includes("echo")) return "demo";
  return "custom";
}

export function gameMeta(slug?: string, name?: string, game?: string | null): GameMeta {
  return BY_KEY.get(inferGameKey(slug, name, game)) ?? BY_KEY.get("custom")!;
}

export function GameIcon({
  slug,
  name,
  game,
  size = "md",
}: {
  slug?: string;
  name?: string;
  game?: string | null;
  size?: "sm" | "md" | "lg";
}) {
  const meta = gameMeta(slug, name, game);
  const px = size === "lg" ? 56 : size === "sm" ? 36 : 44;
  const icon = size === "lg" ? 26 : size === "sm" ? 16 : 20;
  return (
    <span
      className="inline-flex shrink-0 items-center justify-center rounded-xl text-white shadow-[inset_0_1px_0_rgba(255,255,255,0.18)]"
      style={{
        width: px,
        height: px,
        background: `linear-gradient(145deg, ${meta.from}, ${meta.to})`,
      }}
      title={meta.label}
      aria-hidden
    >
      <meta.Icon size={icon} strokeWidth={2.1} />
    </span>
  );
}
