import type { Server, ServerFlavor, ServerRunningInfo, ServerState } from "./types";

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
  const port = serverPort(server);
  return port === DEFAULT_PORT ? "localhost" : `localhost:${port}`;
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
