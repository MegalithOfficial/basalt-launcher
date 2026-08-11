import { cn } from "../../lib/cn";
import { stateLabel } from "../../lib/servers";
import type { Server, ServerRunningInfo } from "../../lib/types";

export function ServerStatusPill({
  server,
  info,
  className,
}: {
  server: Server;
  info: ServerRunningInfo | undefined;
  className?: string;
}) {
  const label = stateLabel(info?.state, server);
  const tone = !server.available
    ? "border-danger/40 bg-danger/10 text-danger"
    : info?.state === "running"
      ? "border-ok/40 bg-ok/10 text-ok"
      : info?.state === "stopping"
        ? "border-warn/40 bg-warn/10 text-warn"
        : info?.state === "crashed"
          ? "border-danger/40 bg-danger/10 text-danger"
          : "border-border bg-surface-2 text-content-muted";

  return (
    <span
      className={cn(
        "inline-flex shrink-0 items-center gap-1.5 rounded-full border px-2.5 py-1 text-[11px] font-medium",
        tone,
        className,
      )}
    >
      <span
        className={cn(
          "size-1.5 rounded-full",
          info?.state === "running"
            ? "bg-ok"
            : info?.state === "stopping"
              ? "bg-warn"
              : !server.available || info?.state === "crashed"
                ? "bg-danger"
                : "bg-content-faint",
        )}
      />
      {label}
    </span>
  );
}
