import type {
  ConsoleLine,
  Server,
  ServerFlavor,
  ServerRunningInfo,
  ServerState,
} from "./types";

export const FLAVORS: Array<{ id: ServerFlavor; label: string; hint: string }> = [
  { id: "vanilla", label: "Vanilla", hint: "Mojang's own server, no mods or plugins." },
  { id: "paper", label: "Paper", hint: "Faster vanilla with a plugin ecosystem." },
  { id: "purpur", label: "Purpur", hint: "Paper with a lot more knobs to turn." },
  { id: "fabric", label: "Fabric", hint: "Lightweight mod loader." },
  { id: "neoforge", label: "NeoForge", hint: "The loader most modern modpacks use." },
  { id: "forge", label: "Forge", hint: "The original loader, still everywhere." },
];

export const DEFAULT_PORT = 25565;

export function flavorLabel(flavor: ServerFlavor): string {
  return FLAVORS.find((entry) => entry.id === flavor)?.label ?? flavor;
}

export function takesPlugins(flavor: ServerFlavor): boolean {
  return flavor === "paper" || flavor === "purpur";
}

export function takesMods(flavor: ServerFlavor): boolean {
  return flavor === "fabric" || flavor === "neoforge" || flavor === "forge";
}

export function needsFlavorVersion(flavor: ServerFlavor): boolean {
  return flavor !== "vanilla";
}

export function isLive(info: ServerRunningInfo | undefined): boolean {
  return info?.state === "running" || info?.state === "stopping";
}

export function serverPort(server: Server): number {
  return server.port ?? DEFAULT_PORT;
}

export function serverAddress(server: Server): string {
  return `localhost:${serverPort(server)}`;
}

export function lanAddress(server: Server, host: string | null): string | null {
  return host ? `${host}:${serverPort(server)}` : null;
}

export function stateLabel(state: ServerState | undefined, server: Server): string {
  if (!server.available) return "Unavailable";
  switch (state) {
    case "running":
      return "Running";
    case "stopping":
      return "Stopping";
    case "crashed":
      return "Crashed";
    default:
      return server.installed_at ? "Stopped" : "Not installed";
  }
}

const TIMESTAMP = /^\[(\d{2}:\d{2}:\d{2})\]\s*(\[[^\]]*\]:?)?\s?/;

export interface ConsoleParts {
  time: string | null;
  source: string | null;
  message: string;
  level: "error" | "warn" | "info";
}

export function readConsoleLine(stream: string, line: string): ConsoleParts {
  if (stream === "input") {
    return { time: null, source: null, message: line, level: "info" };
  }
  const match = TIMESTAMP.exec(line);
  const source = match?.[2] ?? null;
  const upper = (source ?? line).toUpperCase();
  const level = upper.includes("ERROR") || upper.includes("FATAL") || stream === "stderr"
    ? "error"
    : upper.includes("WARN")
      ? "warn"
      : "info";
  return {
    time: match?.[1] ?? null,
    source,
    message: match ? line.slice(match[0].length) : line,
    level,
  };
}

const JOINED = /(?:^|\]:?\s)([A-Za-z0-9_]{3,16}) joined the game\s*$/;
const LEFT = /(?:^|\]:?\s)([A-Za-z0-9_]{3,16}) left the game\s*$/;

export function onlinePlayers(lines: ConsoleLine[]): string[] {
  const online: string[] = [];
  for (const entry of lines) {
    if (entry.stream === "input") continue;
    const joined = JOINED.exec(entry.line);
    if (joined && !online.includes(joined[1])) {
      online.push(joined[1]);
      continue;
    }
    const left = LEFT.exec(entry.line);
    if (left) {
      const at = online.indexOf(left[1]);
      if (at >= 0) online.splice(at, 1);
    }
  }
  return online;
}
