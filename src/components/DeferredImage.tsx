import { useEffect, useRef, useState, type CSSProperties, type ReactNode } from "react";

const MAX_CONCURRENT_IMAGES = 4;

interface QueuedImage {
  cancelled: boolean;
  run: () => void;
}

let activeImages = 0;
const queuedImages: QueuedImage[] = [];
const loadedImages = new Set<string>();

function pumpImageQueue() {
  while (activeImages < MAX_CONCURRENT_IMAGES) {
    const next = queuedImages.shift();
    if (!next) return;
    if (!next.cancelled) next.run();
  }
}

function scheduleImage(start: (release: () => void) => void) {
  let release: (() => void) | null = null;
  const queued: QueuedImage = {
    cancelled: false,
    run: () => {
      activeImages += 1;
      let released = false;
      release = () => {
        if (released) return;
        released = true;
        activeImages -= 1;
        pumpImageQueue();
      };
      start(release);
    },
  };

  queuedImages.push(queued);
  pumpImageQueue();

  return () => {
    queued.cancelled = true;
    release?.();
  };
}

export function DeferredImage({
  src,
  alt = "",
  className,
  style,
  fallback,
}: {
  src: string;
  alt?: string;
  className?: string;
  style?: CSSProperties;
  fallback: ReactNode;
}) {
  const target = useRef<HTMLDivElement>(null);
  const release = useRef<(() => void) | null>(null);
  const [ready, setReady] = useState(false);
  const [failed, setFailed] = useState(false);

  useEffect(() => {
    setReady(false);
    setFailed(false);
    const element = target.current;
    if (!element) return;
    if (loadedImages.has(src)) {
      setReady(true);
      return;
    }

    let cancel: (() => void) | undefined;
    const enqueue = () => {
      cancel ??= scheduleImage((done) => {
        release.current = done;
        setReady(true);
      });
    };

    if (!("IntersectionObserver" in window)) {
      enqueue();
      return () => cancel?.();
    }

    const observer = new IntersectionObserver(
      ([entry]) => {
        if (!entry.isIntersecting) return;
        observer.disconnect();
        enqueue();
      },
      { rootMargin: "300px" },
    );
    observer.observe(element);

    return () => {
      observer.disconnect();
      cancel?.();
      release.current = null;
    };
  }, [src]);

  const finish = () => {
    release.current?.();
    release.current = null;
  };

  return (
    <div ref={target} className="shrink-0">
      {ready && !failed ? (
        <img
          src={src}
          alt={alt}
          className={className}
          style={style}
          loading="lazy"
          decoding="async"
          fetchPriority="low"
          draggable={false}
          onLoad={() => {
            loadedImages.add(src);
            finish();
          }}
          onError={() => {
            finish();
            setFailed(true);
          }}
        />
      ) : (
        fallback
      )}
    </div>
  );
}
