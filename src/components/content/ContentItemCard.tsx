import { ArrowUpCircle, FileBox, Loader2, Trash2 } from "lucide-react";

import { cn } from "../../lib/cn";
import { formatBytes } from "../../lib/format";
import type { ContentItem, SearchProvider } from "../../lib/types";
import { DeferredImage } from "../DeferredImage";

function Toggle({
  on,
  onClick,
  disabled,
}: {
  on: boolean;
  onClick: () => void;
  disabled?: boolean;
}) {
  return (
    <button
      onClick={onClick}
      disabled={disabled}
      aria-label={on ? "Disable" : "Enable"}
      className={cn(
        "relative h-5 w-9 shrink-0 rounded-full transition-colors duration-300",
        on ? "bg-(--accent)" : "bg-surface-3",
        disabled && "cursor-not-allowed opacity-40",
      )}
    >
      <span
        className={cn(
          "absolute left-0.5 top-0.5 size-4 rounded-full bg-white shadow transition-transform duration-300",
          on ? "translate-x-4" : "translate-x-0",
        )}
      />
    </button>
  );
}

function Tag({ tone, title, children }: { tone?: "accent"; title?: string; children: string }) {
  return (
    <span
      title={title}
      className={cn(
        "shrink-0 rounded px-1.5 py-0.5 text-[9px] font-semibold uppercase tracking-wide",
        tone === "accent"
          ? "bg-(--accent-glow) text-content-muted"
          : "bg-surface-3 text-content-faint",
      )}
    >
      {children}
    </span>
  );
}

export function ContentItemCard({
  item,
  busy,
  disabled,
  disabledReason,
  onOpenProject,
  onUpdate,
  onToggle,
  onRemove,
  onContextMenu,
}: {
  item: ContentItem;
  busy?: boolean;
  disabled?: boolean;
  disabledReason?: string;
  onOpenProject?: (provider: SearchProvider, projectId: string, title?: string) => void;
  onUpdate: () => void;
  onToggle: () => void;
  onRemove: () => void;
  onContextMenu?: (event: React.MouseEvent) => void;
}) {
  const source = item.source;
  const displayName = source?.title ?? item.file_name;
  const linked = !!source?.provider && !!source.project_id && !!onOpenProject;

  return (
    <div
      onContextMenu={onContextMenu}
      className={cn(
        "flex items-center gap-3 rounded-xl border px-4 py-2.5 transition-opacity",
        item.update ? "border-warn/30 bg-warn/6" : "border-border-soft bg-surface-2/70",
        !item.enabled && "opacity-55",
      )}
    >
      {source?.icon_url ? (
        <DeferredImage
          src={source.icon_url}
          alt=""
          className="size-9 shrink-0 rounded-lg bg-surface-3 object-cover"
          fallback={
            <div className="grid size-9 shrink-0 place-items-center rounded-lg bg-surface-3 text-content-faint">
              <FileBox className="size-4" />
            </div>
          }
        />
      ) : (
        <div className="grid size-9 shrink-0 place-items-center rounded-lg bg-surface-3 text-content-faint">
          <FileBox className="size-4" />
        </div>
      )}

      <div
        className={cn("min-w-0 flex-1", linked && "cursor-pointer")}
        onClick={() =>
          linked &&
          onOpenProject?.(
            source!.provider! as SearchProvider,
            source!.project_id!,
            source!.title ?? undefined,
          )
        }
      >
        <div className="flex items-center gap-2">
          <span className="truncate text-sm font-medium text-content">{displayName}</span>
          {source?.provider && <Tag>{source.provider}</Tag>}
          {source?.origin === "pack" && <Tag tone="accent">pack</Tag>}
          {source?.origin === "dependency" && <Tag>dependency</Tag>}
          {!linked && source?.mod_id && (
            <Tag title="Identified from the file itself, not linked to a provider">local</Tag>
          )}
        </div>
        <div className="truncate text-[11px] text-content-faint">
          {source?.title ? `${item.file_name} · ` : ""}
          {source?.mod_version && `v${source.mod_version} · `}
          {formatBytes(item.size)}
          {!item.enabled && " · disabled"}
        </div>
      </div>

      {item.update && (
        <button
          onClick={onUpdate}
          disabled={busy || disabled}
          title={disabledReason ?? `Update to ${item.update.latest_name}`}
          className="inline-flex h-8 shrink-0 items-center gap-1.5 rounded-lg bg-warn/15 px-3 text-xs font-semibold text-warn transition-colors hover:bg-warn/25 disabled:cursor-not-allowed disabled:opacity-40"
        >
          {busy ? (
            <Loader2 className="size-3.5 animate-spin" />
          ) : (
            <ArrowUpCircle className="size-3.5" />
          )}
          Update
        </button>
      )}

      <Toggle on={item.enabled} disabled={disabled} onClick={onToggle} />
      <button
        onClick={onRemove}
        disabled={disabled}
        aria-label="Delete file"
        title={disabledReason}
        className="grid size-8 place-items-center rounded-lg text-content-faint transition-colors hover:bg-danger/15 hover:text-danger disabled:cursor-not-allowed disabled:opacity-40 disabled:hover:bg-transparent disabled:hover:text-content-faint"
      >
        <Trash2 className="size-4" />
      </button>
    </div>
  );
}
