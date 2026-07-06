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
  const [failed, setFailed] = useState(false);

  if (failed) {
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
      src={`https://crafatar.com/avatars/${uuid}?size=${size * 2}&overlay`}
      onError={() => setFailed(true)}
      alt={name}
      className={cn("rounded-md [image-rendering:pixelated]", className)}
      style={{ width: size, height: size }}
      draggable={false}
    />
  );
}
