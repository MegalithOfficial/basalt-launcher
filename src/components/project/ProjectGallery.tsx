import { useState } from "react";
import { X } from "lucide-react";

import type { GalleryImage } from "../../lib/types";
import { Modal } from "../Modal";

export function ProjectGallery({ images }: { images: GalleryImage[] }) {
  const [active, setActive] = useState<GalleryImage | null>(null);
  return (
    <>
      <div className="mx-auto grid max-w-5xl grid-cols-[repeat(auto-fill,minmax(280px,1fr))] gap-4 px-6 py-6">
        {images.map((image) => (
          <figure
            key={image.url}
            onClick={() => setActive(image)}
            className="group cursor-zoom-in overflow-hidden rounded-xl border border-border-soft bg-surface-2/40 transition-colors hover:border-content-faint/30"
          >
            <img
              src={image.url}
              loading="lazy"
              className="aspect-video w-full object-cover transition-transform duration-300 group-hover:scale-[1.03]"
              draggable={false}
            />
            {(image.title || image.description) && (
              <figcaption className="px-3 py-2">
                {image.title && (
                  <div className="truncate text-xs font-medium text-content">{image.title}</div>
                )}
                {image.description && (
                  <div className="line-clamp-2 text-[11px] text-content-muted">
                    {image.description}
                  </div>
                )}
              </figcaption>
            )}
          </figure>
        ))}
      </div>

      <Modal
        open={!!active}
        onClose={() => setActive(null)}
        variant="bare"
        nested
        backdropClassName="cursor-zoom-out bg-black/85"
      >
        {active && (
          <>
            <button
              onClick={() => setActive(null)}
              aria-label="Close"
              className="absolute right-6 top-6 grid size-9 place-items-center rounded-full border border-white/10 bg-black/50 text-white/70 transition-colors hover:bg-black/70 hover:text-white"
            >
              <X className="size-4" />
            </button>
            <img
              src={active.raw_url ?? active.url}
              className="max-h-[calc(100vh-9rem)] w-auto max-w-full cursor-default rounded-xl object-contain"
              draggable={false}
            />
            {(active.title || active.description) && (
              <div className="max-w-2xl text-center">
                {active.title && (
                  <div className="text-sm font-medium text-white">{active.title}</div>
                )}
                {active.description && (
                  <div className="mt-1 text-xs text-white/60">{active.description}</div>
                )}
              </div>
            )}
          </>
        )}
      </Modal>
    </>
  );
}
