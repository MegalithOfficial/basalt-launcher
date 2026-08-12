import { stripAnsi } from "./ansi";
import type {
  ConsoleLine,
  Server,
  ServerFlavor,
  ServerRunningInfo,
  ServerSoftware,
  ServerState,
} from "./types";

export const DEFAULT_PORT = 25565;

export function softwareOf(
  software: ServerSoftware[],
  flavor: ServerFlavor,
): ServerSoftware | undefined {
  return software.find((entry) => entry.id === flavor);
}

export function flavorLabel(software: ServerSoftware[], flavor: ServerFlavor): string {
  return softwareOf(software, flavor)?.label ?? flavor;
}

export function needsFlavorVersion(software: ServerSoftware[], flavor: ServerFlavor): boolean {
  return softwareOf(software, flavor)?.builds ?? false;
}

export function isNative(software: ServerSoftware[], flavor: ServerFlavor): boolean {
  return softwareOf(software, flavor)?.runtime === "native";
}

export function contentLabel(
  software: ServerSoftware[],
  flavor: ServerFlavor,
): string | null {
  const dir = softwareOf(software, flavor)?.content_dir;
  if (!dir) return null;
  return dir === "plugins" ? "Plugins" : "Mods";
}

export function configFile(software: ServerSoftware[], flavor: ServerFlavor): string {
  return softwareOf(software, flavor)?.config_file ?? "server.properties";
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

export function readConsoleLine(stream: string, raw: string): ConsoleParts {
  if (stream === "input") {
    return { time: null, source: null, message: raw, level: "info" };
  }
  const line = stripAnsi(raw);
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
    const line = stripAnsi(entry.line);
    const joined = JOINED.exec(line);
    if (joined && !online.includes(joined[1])) {
      online.push(joined[1]);
      continue;
    }
    const left = LEFT.exec(line);
    if (left) {
      const at = online.indexOf(left[1]);
      if (at >= 0) online.splice(at, 1);
    }
  }
  return online;
}

const SERVER_PACK = /(^|[-_. ])server([-_. ]?pack)?([-_. ]|\.|$)/i;

export function serverPackFile<T extends { file_name: string; primary: boolean }>(
  files: T[],
): T | undefined {
  return files.find((file) => !file.primary && SERVER_PACK.test(file.file_name));
}
