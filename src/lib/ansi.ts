export interface AnsiSpan {
  text: string;
  color?: string;
  background?: string;
  bold?: boolean;
  dim?: boolean;
  italic?: boolean;
  underline?: boolean;
}

const ESC = "\u001b";

const SEQUENCE =
  /\u001b(?:\[[0-9;:<=>?]*[ -\/]*[@-~]|\][^\u001b\u0007]*(?:\u0007|\u001b\\)?|[P^_X][^\u001b]*(?:\u001b\\)?|[ -\/]*[0-~])/g;

const CONTROLS = /[\u0000-\u0008\u000b-\u001f\u007f-\u009f]/g;

const MAX_SPANS = 128;

const BASIC = [
  "#4b5162",
  "var(--color-danger)",
  "var(--color-ok)",
  "var(--color-warn)",
  "#6a95f0",
  "#b98ce0",
  "#4fbcd4",
  "var(--color-content-muted)",
];

const BRIGHT = [
  "#6b7285",
  "#ff6b70",
  "#6bdcaa",
  "#ffd166",
  "#8fb4ff",
  "#d3aef5",
  "#7ee0f2",
  "var(--color-content)",
];

function clamp(value: number): number {
  return Math.min(255, Math.max(0, Math.trunc(value) || 0));
}

function rgb(red: number, green: number, blue: number): string {
  return `rgb(${clamp(red)} ${clamp(green)} ${clamp(blue)})`;
}

function cube(index: number): string | undefined {
  if (!Number.isInteger(index) || index < 0 || index > 255) return undefined;
  if (index < 8) return BASIC[index];
  if (index < 16) return BRIGHT[index - 8];
  if (index < 232) {
    const level = (step: number) => (step === 0 ? 0 : step * 40 + 55);
    const offset = index - 16;
    return rgb(
      level(Math.floor(offset / 36) % 6),
      level(Math.floor(offset / 6) % 6),
      level(offset % 6),
    );
  }
  const grey = (index - 232) * 10 + 8;
  return rgb(grey, grey, grey);
}

function extended(codes: number[], at: number): { color?: string; next: number } {
  if (codes[at] === 5) return { color: cube(codes[at + 1]), next: at + 2 };
  if (codes[at] === 2) {
    return { color: rgb(codes[at + 1], codes[at + 2], codes[at + 3]), next: at + 4 };
  }
  return { next: at + 1 };
}

function apply(style: AnsiSpan, codes: number[]): AnsiSpan {
  let next: AnsiSpan = { ...style, text: "" };
  for (let at = 0; at < codes.length; at += 1) {
    const code = codes[at];
    if (code === 0) {
      next = { text: "" };
    } else if (code === 1) {
      next.bold = true;
    } else if (code === 2) {
      next.dim = true;
    } else if (code === 3) {
      next.italic = true;
    } else if (code === 4) {
      next.underline = true;
    } else if (code === 22) {
      next.bold = undefined;
      next.dim = undefined;
    } else if (code === 23) {
      next.italic = undefined;
    } else if (code === 24) {
      next.underline = undefined;
    } else if (code >= 30 && code <= 37) {
      next.color = BASIC[code - 30];
    } else if (code >= 90 && code <= 97) {
      next.color = BRIGHT[code - 90];
    } else if (code === 39) {
      next.color = undefined;
    } else if (code >= 40 && code <= 47) {
      next.background = BASIC[code - 40];
    } else if (code >= 100 && code <= 107) {
      next.background = BRIGHT[code - 100];
    } else if (code === 49) {
      next.background = undefined;
    } else if (code === 38 || code === 48) {
      const found = extended(codes, at + 1);
      if (code === 38) next.color = found.color;
      else next.background = found.color;
      at = found.next - 1;
    }
  }
  return next;
}

function readable(text: string): string {
  return text.replace(CONTROLS, "");
}

function sameStyle(left: AnsiSpan, right: AnsiSpan): boolean {
  return (
    left.color === right.color &&
    left.background === right.background &&
    left.bold === right.bold &&
    left.dim === right.dim &&
    left.italic === right.italic &&
    left.underline === right.underline
  );
}

export function hasAnsi(line: string): boolean {
  return line.includes(ESC);
}

export function stripAnsi(line: string): string {
  return readable(line.replace(SEQUENCE, ""));
}

export function parseAnsi(line: string): AnsiSpan[] {
  const spans: AnsiSpan[] = [];
  let style: AnsiSpan = { text: "" };
  let cursor = 0;

  const push = (text: string) => {
    const clean = readable(text);
    if (!clean) return;
    const last = spans[spans.length - 1];
    if (last && (sameStyle(last, style) || spans.length >= MAX_SPANS)) {
      last.text += clean;
      return;
    }
    spans.push({ ...style, text: clean });
  };

  SEQUENCE.lastIndex = 0;
  for (let found = SEQUENCE.exec(line); found; found = SEQUENCE.exec(line)) {
    push(line.slice(cursor, found.index));
    cursor = found.index + found[0].length;

    const sequence = found[0];
    if (!sequence.startsWith(`${ESC}[`) || !sequence.endsWith("m")) continue;
    const body = sequence.slice(2, -1);
    if (!/^[0-9;]*$/.test(body)) continue;
    style = apply(
      style,
      body
        .split(";")
        .map((part) => (part === "" ? 0 : Number(part)))
        .filter((code) => Number.isInteger(code) && code >= 0),
    );
  }
  push(line.slice(cursor));

  return spans;
}
