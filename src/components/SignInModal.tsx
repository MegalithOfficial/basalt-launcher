import { useState } from "react";
import { AnimatePresence, motion } from "motion/react";
import { openUrl } from "@tauri-apps/plugin-opener";
import { Check, Copy, ExternalLink, Loader2, TriangleAlert, X } from "lucide-react";

import { Button } from "./ui";
import { useEscape } from "../lib/useEscape";
import { useStore } from "../store";

export function SignInModal() {
  const auth = useStore((s) => s.auth);
  const resetAuth = useStore((s) => s.resetAuth);
  const [copied, setCopied] = useState(false);

  const open = auth.status !== "idle";
  useEscape(open, resetAuth);

  const copy = async () => {
    if (!auth.userCode) return;
    await navigator.clipboard.writeText(auth.userCode);
    setCopied(true);
    setTimeout(() => setCopied(false), 1500);
  };

  return (
    <AnimatePresence>
      {open && (
        <motion.div
          initial={{ opacity: 0 }}
          animate={{ opacity: 1 }}
          exit={{ opacity: 0 }}
          className="fixed inset-0 z-50 grid place-items-center bg-black/60 p-6 backdrop-blur-sm"
          onClick={resetAuth}
        >
          <motion.div
            initial={{ opacity: 0, scale: 0.97, y: 8 }}
            animate={{ opacity: 1, scale: 1, y: 0 }}
            exit={{ opacity: 0, scale: 0.97, y: 8 }}
            transition={{ duration: 0.18 }}
            onClick={(e) => e.stopPropagation()}
            role="dialog"
            aria-modal="true"
            aria-label="Sign in with Microsoft"
            className="relative w-full max-w-md overflow-hidden rounded-2xl border border-border bg-surface shadow-2xl"
          >
            <div className="flex items-center justify-between border-b border-border-soft px-5 py-4">
              <h2 className="font-display text-lg font-semibold text-content">
                Sign in with Microsoft
              </h2>
              <button
                onClick={resetAuth}
                aria-label="Close"
                className="grid size-7 place-items-center rounded-md text-content-faint hover:bg-surface-2 hover:text-content"
              >
                <X className="size-4" />
              </button>
            </div>

            {auth.status === "starting" && (
              <div className="flex items-center justify-center gap-2 px-5 py-12 text-sm text-content-muted">
                <Loader2 className="size-4 animate-spin" />
                Starting sign-in
              </div>
            )}

            {auth.status === "error" && (
              <div className="px-5 py-6 text-center">
                <TriangleAlert className="mx-auto size-6 text-danger" />
                <div className="mt-2 font-display font-semibold text-content">
                  Sign-in failed
                </div>
                <p className="mt-1 break-words text-sm text-content-muted">
                  {auth.message}
                </p>
                <Button variant="ghost" className="mt-5" onClick={resetAuth}>
                  Close
                </Button>
              </div>
            )}

            {auth.status === "pending" && (
              <div className="px-5 py-6">
                <p className="text-sm text-content-muted">
                  Open the sign-in page and enter this code to link your account.
                </p>

                <button
                  onClick={copy}
                  title="Copy the code"
                  className="group mt-4 flex w-full items-center justify-center gap-3 rounded-xl border border-border bg-base px-5 py-4 transition-colors hover:border-[var(--accent)]"
                >
                  <span className="font-mono text-2xl font-bold tracking-[0.25em] text-content">
                    {auth.userCode}
                  </span>
                  {copied ? (
                    <Check className="size-4 text-ok" />
                  ) : (
                    <Copy className="size-4 text-content-faint group-hover:text-content" />
                  )}
                </button>

                <Button
                  className="mt-4 w-full"
                  onClick={() => auth.verificationUri && openUrl(auth.verificationUri)}
                >
                  <ExternalLink className="size-4" />
                  Open sign-in page
                </Button>

                <div className="mt-4 flex items-center justify-center gap-2 text-xs text-content-faint">
                  <Loader2 className="size-3 animate-spin" />
                  Waiting for you to finish in the browser
                </div>
              </div>
            )}
          </motion.div>
        </motion.div>
      )}
    </AnimatePresence>
  );
}
