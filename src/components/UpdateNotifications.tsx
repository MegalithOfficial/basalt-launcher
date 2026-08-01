import { useEffect } from "react";
import { openUrl } from "@tauri-apps/plugin-opener";
import { ArrowDownToLine, RefreshCw, X } from "lucide-react";
import { toast } from "sonner";

import { useStore } from "../store";

const AVAILABLE_TOAST = "app-update-available";
const READY_TOAST = "app-update-ready";

const primaryButton =
  "rounded-lg px-3 py-1.5 text-[11px] font-semibold text-black [background:linear-gradient(to_bottom,var(--accent),var(--accent-deep))] hover:[background:linear-gradient(to_bottom,var(--accent-bright),var(--accent))]";
const secondaryButton =
  "rounded-lg border border-border bg-surface-2 px-3 py-1.5 text-[11px] font-medium text-content-muted transition-colors hover:bg-surface-3 hover:text-content";

export function UpdateNotifications() {
  const ready = useStore((state) => state.ready);
  const onboarded = useStore((state) => state.settings?.onboarded !== false);
  const status = useStore((state) => state.appUpdateStatus);
  const dismissUpdate = useStore((state) => state.dismissAppUpdate);
  const downloadUpdate = useStore((state) => state.downloadAppUpdate);
  const installUpdate = useStore((state) => state.installAppUpdate);

  useEffect(() => {
    if (!ready || !onboarded || !status) return;

    if (status.phase !== "available" || status.dismissed || !status.info?.latest) {
      toast.dismiss(AVAILABLE_TOAST);
    }
    if (status.phase !== "ready") toast.dismiss(READY_TOAST);

    const info = status.info;
    if (status.phase === "available" && !status.dismissed && info?.latest) {
      const selfManaged = info.install_source.policy === "self_managed";
      toast.custom(
        () => (
          <div className="w-92 rounded-xl border border-accent/35 bg-surface p-3.5 shadow-2xl">
            <div className="flex items-start gap-3">
              <span className="grid size-9 shrink-0 place-items-center rounded-lg bg-accent/15 text-accent">
                <ArrowDownToLine className="size-4" />
              </span>
              <div className="min-w-0 flex-1">
                <div className="text-[13px] font-semibold text-content">
                  Basalt {info.latest} is available
                </div>
                <div className="mt-0.5 text-[11px] leading-relaxed text-content-muted">
                  {selfManaged
                    ? "Download it in the background and restart when you are ready."
                    : info.install_source.update_hint}
                </div>
              </div>
              <button
                onClick={() => {
                  toast.dismiss(AVAILABLE_TOAST);
                  void dismissUpdate(info.latest!);
                }}
                aria-label="Dismiss this update"
                className="grid size-7 place-items-center rounded-md text-content-faint hover:bg-surface-2 hover:text-content"
              >
                <X className="size-3.5" />
              </button>
            </div>
            <div className="mt-3 flex items-center justify-end gap-2">
              <button
                onClick={() => {
                  toast.dismiss(AVAILABLE_TOAST);
                  void dismissUpdate(info.latest!);
                }}
                className={secondaryButton}
              >
                Dismiss
              </button>
              <button
                onClick={() => {
                  if (selfManaged) {
                    toast.dismiss(AVAILABLE_TOAST);
                    void downloadUpdate().catch((error) =>
                      toast.error("Update download failed", {
                        description: String(error),
                      }),
                    );
                  } else if (info.notes_url) {
                    void openUrl(info.notes_url);
                  }
                }}
                className={primaryButton}
              >
                {selfManaged ? "Update" : "View update"}
              </button>
            </div>
          </div>
        ),
        { id: AVAILABLE_TOAST, duration: Infinity },
      );
    }

    const readyVersion = status.info?.latest;
    if (status.phase === "ready" && readyVersion) {
      toast.custom(
        () => (
          <div className="w-92 rounded-xl border border-ok/35 bg-surface p-3.5 shadow-2xl">
            <div className="flex items-start gap-3">
              <span className="grid size-9 shrink-0 place-items-center rounded-lg bg-ok/15 text-ok">
                <RefreshCw className="size-4" />
              </span>
              <div className="min-w-0 flex-1">
                <div className="text-[13px] font-semibold text-content">Update ready</div>
                <div className="mt-0.5 text-[11px] leading-relaxed text-content-muted">
                  Restart Basalt to install {readyVersion}.
                </div>
              </div>
            </div>
            <div className="mt-3 flex items-center justify-end gap-2">
              <button onClick={() => toast.dismiss(READY_TOAST)} className={secondaryButton}>
                Later
              </button>
              <button
                onClick={() => {
                  void installUpdate().catch((error) =>
                    toast.error("Basalt could not restart for the update", {
                      description: String(error),
                    }),
                  );
                }}
                className={primaryButton}
              >
                Restart and update
              </button>
            </div>
          </div>
        ),
        { id: READY_TOAST, duration: Infinity },
      );
    }
  }, [dismissUpdate, downloadUpdate, installUpdate, onboarded, ready, status]);

  return null;
}
