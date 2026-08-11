import type { FileKind } from "./types";

export interface Token {
  text: string;
  cls: string;
}

const COMMENT = "text-content-faint";
const KEY = "text-content";
const STRING = "text-ok";
const LITERAL = "text-warn";
const PUNCT = "text-content-faint";
const PLAIN = "text-content-muted";

export const MAX_HIGHLIGHTED_LINES = 5000;

export function highlights(kind: FileKind): boolean {
  return kind === "properties" || kind === "json" || kind === "yaml" || kind === "toml";
}

export function highlightLine(kind: FileKind, line: string): Token[] {
  switch (kind) {
    case "properties":
      return properties(line);
    case "yaml":
      return yaml(line);
    case "toml":
      return toml(line);
    case "json":
      return json(line);
    default:
      return [{ text: line, cls: PLAIN }];
  }
}

function comment(line: string, marker: string): Token[] | null {
  const indent = line.length - line.trimStart().length;
  if (!line.slice(indent).startsWith(marker)) return null;
  return [{ text: line, cls: COMMENT }];
}

function properties(line: string): Token[] {
  const hash = comment(line, "#") ?? comment(line, "!");
  if (hash) return hash;
  if (line.trim() === "") return [{ text: line, cls: PLAIN }];

  const separator = line.search(/[=:]/);
  if (separator < 0) return [{ text: line, cls: KEY }];
  return [
    { text: line.slice(0, separator), cls: KEY },
    { text: line[separator], cls: PUNCT },
    { text: line.slice(separator + 1), cls: value(line.slice(separator + 1)) },
  ];
}

function value(raw: string): string {
  const trimmed = raw.trim();
  if (trimmed === "") return PLAIN;
  if (/^(true|false|null|yes|no|on|off)$/i.test(trimmed)) return LITERAL;
  if (/^-?\d+(\.\d+)?$/.test(trimmed)) return LITERAL;
  return STRING;
}

function yaml(line: string): Token[] {
  const hash = comment(line, "#");
  if (hash) return hash;
  if (line.trim() === "") return [{ text: line, cls: PLAIN }];

  const indent = line.length - line.trimStart().length;
  const tokens: Token[] = [];
  let rest = line.slice(indent);
  if (indent > 0) tokens.push({ text: line.slice(0, indent), cls: PLAIN });

  if (rest.startsWith("- ")) {
    tokens.push({ text: "- ", cls: PUNCT });
    rest = rest.slice(2);
  }

  const colon = rest.indexOf(":");
  if (colon < 0) {
    tokens.push({ text: rest, cls: value(rest) });
    return tokens;
  }
  tokens.push({ text: rest.slice(0, colon), cls: KEY });
  tokens.push({ text: ":", cls: PUNCT });
  const tail = rest.slice(colon + 1);
  if (tail !== "") tokens.push({ text: tail, cls: value(tail) });
  return tokens;
}

function toml(line: string): Token[] {
  const hash = comment(line, "#");
  if (hash) return hash;
  const trimmed = line.trim();
  if (trimmed === "") return [{ text: line, cls: PLAIN }];
  if (trimmed.startsWith("[")) return [{ text: line, cls: KEY }];

  const equals = line.indexOf("=");
  if (equals < 0) return [{ text: line, cls: PLAIN }];
  return [
    { text: line.slice(0, equals), cls: KEY },
    { text: "=", cls: PUNCT },
    { text: line.slice(equals + 1), cls: value(line.slice(equals + 1)) },
  ];
}

const JSON_PATTERN = /("(?:\\.|[^"\\])*"\s*:)|("(?:\\.|[^"\\])*")|(-?\d+(?:\.\d+)?(?:[eE][+-]?\d+)?)|(true|false|null)|([{}[\],:])/g;

function json(line: string): Token[] {
  const tokens: Token[] = [];
  let last = 0;
  for (const match of line.matchAll(JSON_PATTERN)) {
    const at = match.index ?? 0;
    if (at > last) tokens.push({ text: line.slice(last, at), cls: PLAIN });
    const [text, key, string, number, literal, punctuation] = match;
    tokens.push({
      text,
      cls: key ? KEY : string ? STRING : number || literal ? LITERAL : punctuation ? PUNCT : PLAIN,
    });
    last = at + text.length;
  }
  if (last < line.length) tokens.push({ text: line.slice(last), cls: PLAIN });
  return tokens;
}
