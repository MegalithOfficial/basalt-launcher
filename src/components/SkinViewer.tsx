import { useEffect, useRef } from "react";
import { IdleAnimation, SkinViewer as Viewer, WalkingAnimation } from "skinview3d";

import { cn } from "../lib/cn";

export function SkinViewer({
  skinUrl,
  capeUrl,
  slim,
  walking,
  className,
}: {
  skinUrl: string | null;
  capeUrl?: string | null;
  slim: boolean;
  walking?: boolean;
  className?: string;
}) {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const viewerRef = useRef<Viewer | null>(null);

  useEffect(() => {
    if (!canvasRef.current) return;
    const viewer = new Viewer({
      canvas: canvasRef.current,
      width: 240,
      height: 320,
    });
    viewer.controls.enableZoom = false;
    viewer.controls.enablePan = false;
    viewer.autoRotate = false;
    viewer.zoom = 0.86;
    viewerRef.current = viewer;

    const resize = () => {
      const parent = canvasRef.current?.parentElement;
      if (!parent) return;
      viewer.setSize(parent.clientWidth, parent.clientHeight);
    };
    resize();
    const observer = new ResizeObserver(resize);
    if (canvasRef.current.parentElement) {
      observer.observe(canvasRef.current.parentElement);
    }

    return () => {
      observer.disconnect();
      viewer.dispose();
      viewerRef.current = null;
    };
  }, []);

  useEffect(() => {
    const viewer = viewerRef.current;
    if (!viewer) return;
    if (skinUrl) {
      void viewer.loadSkin(skinUrl, { model: slim ? "slim" : "default" });
    } else {
      viewer.resetSkin();
    }
  }, [skinUrl, slim]);

  useEffect(() => {
    const viewer = viewerRef.current;
    if (!viewer) return;
    if (capeUrl) {
      void viewer.loadCape(capeUrl);
    } else {
      viewer.resetCape();
    }
  }, [capeUrl]);

  useEffect(() => {
    const viewer = viewerRef.current;
    if (!viewer) return;
    viewer.animation = walking ? new WalkingAnimation() : new IdleAnimation();
  }, [walking]);

  return (
    <div className={cn("relative w-full", className)}>
      <canvas ref={canvasRef} className="size-full cursor-grab active:cursor-grabbing" />
    </div>
  );
}
