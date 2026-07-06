import { useState } from "react";

import { cn } from "../lib/cn";

export function PlayerHead({
  uuid,
  name,
  size = 28,
  className,
}: {
  uuid: string;
  name: string;
  size?: number;
  className?: string;
}) {
  const sources = [
    `https://mc-heads.net/avatar/${uuid}/${size * 2}`,
    `https://minotar.net/helm/${uuid}/${size * 2}.png`,
  ];
  const [sourceIndex, setSourceIndex] = useState(0);

  if (sourceIndex >= sources.length) {
    return (
      <span
        className={cn(
          "grid place-items-center rounded-md bg-surface-3 font-bold text-content",
          className,
        )}
        style={{ width: size, height: size, fontSize: size * 0.4 }}
      >
        {name.slice(0, 1).toUpperCase()}
      </span>
    );
  }

  return (
    <img
      src={sources[sourceIndex]}
      onError={() => setSourceIndex((i) => i + 1)}
      alt={name}
      className={cn("rounded-md [image-rendering:pixelated]", className)}
      style={{ width: size, height: size }}
      draggable={false}
    />
  );
}
