import { useEffect, useState } from "react";
import { openUrl } from "@tauri-apps/plugin-opener";
import { Check, Copy, ExternalLink, Loader2, ShieldCheck, TriangleAlert, Upload } from "lucide-react";

import { api } from "../lib/api";
import { log } from "../lib/log";
import { Modal, ModalFooter, ModalHeader } from "./Modal";

export function ShareLogModal({
  open,
  onClose,
  title,
  load,
}: {
  open: boolean;
  onClose: () => void;
  title: string;
  load: () => Promise<string>;
}) {
  const [text, setText] = useState("");
  const [reading, setReading] = useState(true);
  const [uploading, setUploading] = useState(false);
  const [link, setLink] = useState<string | null>(null);
  const [copied, setCopied] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!open) return;
    let live = true;
    setReading(true);
    setLink(null);
    setError(null);
    setCopied(false);
    load()
      .then((value) => live && setText(value))
      .catch((cause) => live && setError(String(cause)))
      .finally(() => live && setReading(false));
    return () => {
      live = false;
    };
  }, [open, load]);

  const upload = async () => {
    setUploading(true);
    setError(null);
    try {
      setLink(await api.shareLog(text));
    } catch (cause) {
      setError(String(cause));
    } finally {
      setUploading(false);
    }
  };

  const copy = async () => {
    if (!link) return;
    try {
      await navigator.clipboard.writeText(link);
      setCopied(true);
      setTimeout(() => setCopied(false), 1600);
    } catch (cause) {
      log.warn("logs", `could not copy the link: ${String(cause)}`);
    }
  };

  const removed = text.split("[redacted]").length - 1;

  return (
    <Modal
      open={open}
      onClose={onClose}
      size="wide"
      dismissable={!uploading}
      className="h-[min(680px,calc(100vh-48px))]"
      labelledBy="share-log-title"
    >
      <ModalHeader
        id="share-log-title"
        title="Share this log"
        subtitle={`${title} goes to mclo.gs, where anyone with the link can read it`}
        icon={
          <div className="grid size-9 place-items-center rounded-xl border border-border-soft bg-surface-2 text-(--accent)">
            <Upload className="size-4" />
          </div>
        }
        onClose={uploading ? undefined : onClose}
      />

      <div className="flex shrink-0 items-start gap-2.5 border-b border-border-soft px-5 py-3">
        <ShieldCheck className="mt-0.5 size-4 shrink-0 text-ok" />
        <p className="text-[11px] leading-relaxed text-content-muted">
          Your account tokens, session id, API keys and home folder name are stripped before
          anything leaves this machine.{" "}
          {removed > 0 && (
            <span className="text-content">
              {removed} {removed === 1 ? "value was" : "values were"} removed below.
            </span>
          )}{" "}
          What you see here is exactly what gets uploaded, so read it first.
        </p>
      </div>

      <div className="flex min-h-0 flex-1 flex-col overflow-hidden">
        {reading ? (
          <div className="flex flex-1 items-center justify-center gap-2 text-sm text-content-muted">
            <Loader2 className="size-4 animate-spin" />
            Reading the log
          </div>
        ) : (
          <pre className="selectable h-full overflow-auto whitespace-pre-wrap break-words bg-void px-4 py-3 font-mono text-[11px] leading-relaxed text-content-muted">
            {text || "This log is empty."}
          </pre>
        )}
      </div>

      {error && (
        <div className="flex shrink-0 gap-2.5 border-t border-danger/25 bg-danger/[0.07] px-5 py-3 text-xs text-danger">
          <TriangleAlert className="mt-0.5 size-4 shrink-0" />
          <span className="break-words">{error}</span>
        </div>
      )}

      <ModalFooter className="justify-between">
        {link ? (
          <a
            href={link}
            onClick={(event) => {
              event.preventDefault();
              void openUrl(link);
            }}
            className="inline-flex min-w-0 items-center gap-1.5 font-mono text-xs text-(--accent) hover:underline"
          >
            <span className="truncate">{link}</span>
            <ExternalLink className="size-3.5 shrink-0" />
          </a>
        ) : (
          <span className="text-[11px] text-content-faint">
            Pastes on mclo.gs expire after 90 days without a view.
          </span>
        )}

        <div className="flex shrink-0 items-center gap-2">
          <button
            onClick={onClose}
            disabled={uploading}
            className="rounded-lg px-3 py-2 text-sm font-medium text-content-muted transition-colors hover:text-content disabled:opacity-50"
          >
            {link ? "Done" : "Cancel"}
          </button>
          {link ? (
            <button
              onClick={() => void copy()}
              className="inline-flex items-center gap-1.5 rounded-lg px-4 py-2 text-sm font-semibold text-black shadow-md shadow-(color:--accent-glow) transition-all [background:linear-gradient(to_bottom,var(--accent),var(--accent-deep))] hover:[background:linear-gradient(to_bottom,var(--accent-bright),var(--accent))] disabled:cursor-not-allowed disabled:opacity-45"
            >
              {copied ? <Check className="size-3.5" /> : <Copy className="size-3.5" />}
              {copied ? "Copied" : "Copy link"}
            </button>
          ) : (
            <button
              onClick={() => void upload()}
              disabled={uploading || reading || !text.trim()}
              className="inline-flex items-center gap-1.5 rounded-lg px-4 py-2 text-sm font-semibold text-black shadow-md shadow-(color:--accent-glow) transition-all [background:linear-gradient(to_bottom,var(--accent),var(--accent-deep))] hover:[background:linear-gradient(to_bottom,var(--accent-bright),var(--accent))] disabled:cursor-not-allowed disabled:opacity-45"
            >
              {uploading ? (
                <Loader2 className="size-3.5 animate-spin" />
              ) : (
                <Upload className="size-3.5" />
              )}
              {uploading ? "Uploading" : "Upload to mclo.gs"}
            </button>
          )}
        </div>
      </ModalFooter>
    </Modal>
  );
}
