import type { AccentMode, LauncherSettings } from "./types";

export const LAVA = "#ff6a2b";

export const DEFAULTS = {
  accent: LAVA,
  ok: "#46c08a",
  warn: "#e9b949",
  danger: "#e5484d",
};

export const ACCENT_PRESETS = [
  "#ff6a2b",
  "#f2545b",
  "#e8a33d",
  "#46c08a",
  "#3aa5d9",
  "#7c6cf0",
  "#c86bd8",
  "#9aa1b0",
];

export function isHex(value: string | null | undefined): boolean {
  return /^#[0-9a-fA-F]{6}$/.test((value ?? "").trim());
}

function hexToRgb(hex: string): [number, number, number] {
  const value = hex.replace("#", "");
  return [
    parseInt(value.slice(0, 2), 16),
    parseInt(value.slice(2, 4), 16),
    parseInt(value.slice(4, 6), 16),
  ];
}

function scale(rgb: [number, number, number], factor: number): string {
  const [r, g, b] = rgb.map((c) => Math.round(Math.min(255, Math.max(0, c * factor))));
  return `rgb(${r}, ${g}, ${b})`;
}

export function accentVars(accent: string | null | undefined): Record<string, string> {
  const hex = isHex(accent) ? (accent as string) : LAVA;
  const rgb = hexToRgb(hex);
  return {
    "--accent": hex,
    "--accent-bright": scale(rgb, 1.18),
    "--accent-deep": scale(rgb, 0.72),
    "--accent-glow": `rgba(${rgb[0]}, ${rgb[1]}, ${rgb[2]}, 0.28)`,
  };
}

export function resolveAccent(
  mode: AccentMode | undefined,
  custom: string | undefined,
  banner: string | null,
): string {
  if (mode === "default") return LAVA;
  if (mode === "custom") return isHex(custom) ? (custom as string) : LAVA;
  return banner ?? LAVA;
}

export function themeVars(
  settings: LauncherSettings | null,
  banner: string | null,
): Record<string, string> {
  return {
    ...accentVars(resolveAccent(settings?.accent_mode, settings?.accent_color, banner)),
    "--color-ok": isHex(settings?.ok_color) ? settings!.ok_color : DEFAULTS.ok,
    "--color-warn": isHex(settings?.warn_color) ? settings!.warn_color : DEFAULTS.warn,
    "--color-danger": isHex(settings?.danger_color) ? settings!.danger_color : DEFAULTS.danger,
  };
}

export function applyTheme(vars: Record<string, string>) {
  const root = document.documentElement;
  for (const [name, value] of Object.entries(vars)) {
    root.style.setProperty(name, value);
  }
}
