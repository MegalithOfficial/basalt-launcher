import { useEffect, useRef } from "react";

import { cn } from "../lib/cn";

export interface CropRect {
  x: number;
  y: number;
  w: number;
  h: number;
}

const REFERENCE_WIDTH = 64;

export const FACE: CropRect = { x: 8, y: 8, w: 8, h: 8 };
export const FACE_OVERLAY: CropRect = { x: 40, y: 8, w: 8, h: 8 };
export const CAPE_FRONT: CropRect = { x: 1, y: 1, w: 10, h: 16 };

function isFilledWithOneColour(
  image: HTMLImageElement,
  rect: CropRect,
  scale: number,
) {
  const probe = document.createElement("canvas");
  probe.width = rect.w;
  probe.height = rect.h;
  const probeCtx = probe.getContext("2d", { willReadFrequently: true });
  if (!probeCtx) return false;
  probeCtx.imageSmoothingEnabled = false;
  probeCtx.drawImage(
    image,
    rect.x * scale,
    rect.y * scale,
    rect.w * scale,
    rect.h * scale,
    0,
    0,
    rect.w,
    rect.h,
  );

  let pixels: Uint8ClampedArray;
  try {
    pixels = probeCtx.getImageData(0, 0, rect.w, rect.h).data;
  } catch {
    return false;
  }
  for (let i = 4; i < pixels.length; i += 4) {
    if (
      pixels[i] !== pixels[0] ||
      pixels[i + 1] !== pixels[1] ||
      pixels[i + 2] !== pixels[2] ||
      pixels[i + 3] !== pixels[3]
    ) {
      return false;
    }
  }
  return pixels[3] === 255;
}

export function TextureCrop({
  url,
  crop,
  overlay,
  className,
  style,
}: {
  url: string;
  crop: CropRect;
  overlay?: CropRect;
  className?: string;
  style?: React.CSSProperties;
}) {
  const canvasRef = useRef<HTMLCanvasElement>(null);

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    let cancelled = false;

    const image = new Image();
    image.crossOrigin = "anonymous";
    image.onload = () => {
      if (cancelled) return;
      const scale = image.naturalWidth / REFERENCE_WIDTH || 1;
      canvas.width = crop.w;
      canvas.height = crop.h;
      const ctx = canvas.getContext("2d");
      if (!ctx) return;
      ctx.imageSmoothingEnabled = false;
      ctx.clearRect(0, 0, crop.w, crop.h);
      const draw = (rect: CropRect) =>
        ctx.drawImage(
          image,
          rect.x * scale,
          rect.y * scale,
          rect.w * scale,
          rect.h * scale,
          0,
          0,
          crop.w,
          crop.h,
        );
      draw(crop);
      if (overlay && !isFilledWithOneColour(image, overlay, scale)) {
        draw(overlay);
      }
    };
    image.src = url;

    return () => {
      cancelled = true;
    };
  }, [url, crop, overlay]);

  return (
    <canvas
      ref={canvasRef}
      className={cn("[image-rendering:pixelated]", className)}
      style={style}
      aria-hidden
    />
  );
}
