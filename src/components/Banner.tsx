import { useEffect, useState } from "react";

import { cn } from "../lib/cn";
import { log } from "../lib/log";
import { mediaSrc } from "../lib/media";
import type { VersionMedia } from "../lib/types";

export function Banner({
  media,
  className,
  still,
}: {
  media: VersionMedia;
  className?: string;
  still?: boolean;
}) {
  const src = mediaSrc(media);
  const [failed, setFailed] = useState(false);

  useEffect(() => setFailed(false), [src]);

  if (media.kind === "video" && !failed && !still) {
    return (
      <video
        src={src}
        autoPlay
        loop
        muted
        playsInline
        disablePictureInPicture
        onError={() => {
          setFailed(true);
          log.warn("media", `the webview could not play ${src}`);
        }}
        className={cn("object-cover", className)}
      />
    );
  }

  if (media.kind === "video") {
    return <div className={cn("bg-surface-2", className)} />;
  }

  return (
    <img
      src={src}
      draggable={false}
      className={cn("object-cover", !media.local && "[image-rendering:pixelated]", className)}
    />
  );
}
