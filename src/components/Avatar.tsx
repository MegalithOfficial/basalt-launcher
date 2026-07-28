import { useState } from "react";

import { cn } from "../lib/cn";
import { useStore } from "../store";
import { FACE, FACE_OVERLAY, TextureCrop } from "./TextureCrop";

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
  const revision = useStore((s) => s.skinRevision);
  const localSkin = useStore((s) => s.skinHeads[uuid]);
  const sources = [
    `https://mc-heads.net/avatar/${uuid}/${size * 2}?v=${revision}`,
    `https://minotar.net/helm/${uuid}/${size * 2}.png?v=${revision}`,
  ];
  const [sourceIndex, setSourceIndex] = useState(0);

  if (localSkin) {
    return (
      <TextureCrop
        url={localSkin}
        crop={FACE}
        overlay={FACE_OVERLAY}
        className={cn("rounded-md", className)}
        style={{ width: size, height: size }}
      />
    );
  }

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
      key={revision}
      src={sources[sourceIndex]}
      onError={() => setSourceIndex((i) => i + 1)}
      alt={name}
      className={cn("rounded-md [image-rendering:pixelated]", className)}
      style={{ width: size, height: size }}
      draggable={false}
    />
  );
}
