import { toast } from "sonner";
import { Check, Package, Trash2, TriangleAlert } from "lucide-react";

import { cn } from "./cn";
import { logoSrc } from "./media";
import type { InstalledItem, Instance, Task } from "./types";

function Thumb({ task, tone }: { task: Task; tone: "success" | "error" }) {
  const success = tone === "success";
  return (
    <span className="relative shrink-0">
      {task.icon_url ? (
        <img
          src={task.icon_url}
          alt=""
          className="size-9 rounded-lg bg-surface-3 object-cover"
          draggable={false}
        />
      ) : (
        <span className="grid size-9 place-items-center rounded-lg bg-surface-3 text-content-faint">
          <Package className="size-4" />
        </span>
      )}
      <span
        className={cn(
          "absolute -bottom-1 -right-1 grid size-4 place-items-center rounded-full ring-2 ring-surface",
          success ? "bg-ok text-black" : "bg-danger text-white",
        )}
      >
        {success ? (
          <Check className="size-2.5" strokeWidth={3} />
        ) : (
          <TriangleAlert className="size-2.5" strokeWidth={3} />
        )}
      </span>
    </span>
  );
}

export function notifyTaskFinished(task: Task) {
  const success = task.state === "succeeded";
  const description = success ? (task.subtitle ?? "Installed") : task.error;

  toast.custom(
    (id) => (
      <div
        onClick={() => toast.dismiss(id)}
        className={cn(
          "flex w-88 cursor-pointer items-center gap-3 rounded-xl border bg-surface p-3 shadow-2xl transition-colors hover:bg-surface-2",
          success ? "border-ok/30" : "border-danger/40",
        )}
      >
        <Thumb task={task} tone={success ? "success" : "error"} />
        <div className="min-w-0 flex-1">
          <div className="truncate text-[13px] font-semibold text-content">
            {success ? task.title : `${task.title} failed`}
          </div>
          {description && (
            <div
              className={cn(
                "mt-0.5 line-clamp-2 text-[11px]",
                success ? "text-content-muted" : "text-danger/90",
              )}
            >
              {description}
            </div>
          )}
        </div>
        <span
          className={cn(
            "h-9 w-1 shrink-0 rounded-full",
            success ? "bg-ok/60" : "bg-danger/70",
          )}
        />
      </div>
    ),
    { id: `task:${task.id}`, duration: success ? 4500 : 11000 },
  );
}

export function notifyInstalled(items: InstalledItem[], into: string) {
  for (const item of items) {
    toast.custom(
      (id) => (
        <div
          onClick={() => toast.dismiss(id)}
          className="flex w-88 cursor-pointer items-center gap-3 rounded-xl border border-ok/30 bg-surface p-3 shadow-2xl transition-colors hover:bg-surface-2"
        >
          <span className="relative shrink-0">
            {item.icon_url ? (
              <img
                src={item.icon_url}
                alt=""
                className="size-9 rounded-lg bg-surface-3 object-cover"
                draggable={false}
              />
            ) : (
              <span className="grid size-9 place-items-center rounded-lg bg-surface-3 text-content-faint">
                <Package className="size-4" />
              </span>
            )}
            <span className="absolute -bottom-1 -right-1 grid size-4 place-items-center rounded-full bg-ok text-black ring-2 ring-surface">
              <Check className="size-2.5" strokeWidth={3} />
            </span>
          </span>
          <div className="min-w-0 flex-1">
            <div className="truncate text-[13px] font-semibold text-content">{item.title}</div>
            <div className="mt-0.5 truncate text-[11px] text-content-muted">
              {item.is_dependency ? `Dependency · into ${into}` : `into ${into}`}
            </div>
          </div>
          <span className="h-9 w-1 shrink-0 rounded-full bg-ok/60" />
        </div>
      ),
      { id: `installed:${into}:${item.file_name}`, duration: 4500 },
    );
  }
}

export function notifyPackImported(instance: Instance, onOpen: () => void) {
  const logo = logoSrc(instance.logo);

  toast.custom(
    (id) => (
      <div
        onClick={() => {
          toast.dismiss(id);
          onOpen();
        }}
        className="flex w-88 cursor-pointer items-center gap-3 rounded-xl border border-ok/30 bg-surface p-3 shadow-2xl transition-colors hover:bg-surface-2"
      >
        <span className="relative shrink-0">
          {logo ? (
            <img
              src={logo}
              alt=""
              className="size-9 rounded-lg bg-surface-3 object-cover"
              draggable={false}
            />
          ) : (
            <span className="grid size-9 place-items-center rounded-lg bg-surface-3 text-content-faint">
              <Package className="size-4" />
            </span>
          )}
          <span className="absolute -bottom-1 -right-1 grid size-4 place-items-center rounded-full bg-ok text-black ring-2 ring-surface">
            <Check className="size-2.5" strokeWidth={3} />
          </span>
        </span>
        <div className="min-w-0 flex-1">
          <div className="truncate text-[13px] font-semibold text-content">
            {instance.name} was imported
          </div>
          <div className="mt-0.5 truncate text-[11px] text-content-muted">
            {instance.version_id}
            {instance.loader ? ` · ${instance.loader}` : ""} · downloading in the background
          </div>
        </div>
        <span className="h-9 w-1 shrink-0 rounded-full bg-ok/60" />
      </div>
    ),
    { id: `imported:${instance.id}`, duration: 6000 },
  );
}

export function notifyRemoved(title: string, description?: string) {
  toast.custom(
    (id) => (
      <div
        onClick={() => toast.dismiss(id)}
        className="flex w-88 cursor-pointer items-center gap-3 rounded-xl border border-border bg-surface p-3 shadow-2xl transition-colors hover:bg-surface-2"
      >
        <span className="grid size-9 shrink-0 place-items-center rounded-lg bg-surface-3 text-content-muted">
          <Trash2 className="size-4" />
        </span>
        <div className="min-w-0 flex-1">
          <div className="truncate text-[13px] font-semibold text-content">{title}</div>
          {description && (
            <div className="mt-0.5 truncate text-[11px] text-content-muted">{description}</div>
          )}
        </div>
        <span className="h-9 w-1 shrink-0 rounded-full bg-border" />
      </div>
    ),
    { duration: 4000 },
  );
}

export function notifySummary(message: string) {
  toast.custom(
    (id) => (
      <div
        onClick={() => toast.dismiss(id)}
        className="flex w-88 cursor-pointer items-center gap-3 rounded-xl border border-ok/30 bg-surface p-3 shadow-2xl transition-colors hover:bg-surface-2"
      >
        <span className="grid size-9 shrink-0 place-items-center rounded-lg bg-ok/15 text-ok">
          <Check className="size-4" strokeWidth={3} />
        </span>
        <div className="min-w-0 flex-1 truncate text-[13px] font-semibold text-content">
          {message}
        </div>
        <span className="h-9 w-1 shrink-0 rounded-full bg-ok/60" />
      </div>
    ),
    { duration: 4500 },
  );
}
