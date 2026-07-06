export const LAVA = "#ff6a2b";

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

export function accentVars(accent: string | null | undefined): React.CSSProperties {
  const hex = accent ?? LAVA;
  const rgb = hexToRgb(hex);
  return {
    "--accent": hex,
    "--accent-bright": scale(rgb, 1.18),
    "--accent-deep": scale(rgb, 0.72),
    "--accent-glow": `rgba(${rgb[0]}, ${rgb[1]}, ${rgb[2]}, 0.28)`,
  } as React.CSSProperties;
}
