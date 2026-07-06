import { useState } from "react";
import { AnimatePresence, motion } from "motion/react";
import { openUrl } from "@tauri-apps/plugin-opener";
import {
  Check,
  Copy,
  ExternalLink,
  Loader2,
  LogIn,
  Trash2,
  TriangleAlert,
  UserCircle2,
} from "lucide-react";

import { cn } from "../lib/cn";
import { Button, EmptyState, PageHeader } from "../components/ui";
import { PlayerHead } from "../components/Avatar";
import { useStore } from "../store";

function DeviceCodeCard() {
  const auth = useStore((s) => s.auth);
  const resetAuth = useStore((s) => s.resetAuth);
  const [copied, setCopied] = useState(false);

  if (auth.status === "idle") return null;

  if (auth.status === "error") {
    return (
      <div className="mx-auto mt-6 w-full max-w-md rounded-xl border border-danger/30 bg-danger/10 p-5 text-center">
        <TriangleAlert className="mx-auto size-6 text-danger" />
        <div className="mt-2 font-display font-semibold text-content">Sign-in failed</div>
        <p className="mt-1 text-sm text-content-muted">{auth.message}</p>
        <Button variant="ghost" className="mt-4" onClick={resetAuth}>
          Try again
        </Button>
      </div>
    );
  }

  const copy = async () => {
    if (!auth.userCode) return;
    await navigator.clipboard.writeText(auth.userCode);
    setCopied(true);
    setTimeout(() => setCopied(false), 1500);
  };

  return (
    <motion.div
      initial={{ opacity: 0, y: 8 }}
      animate={{ opacity: 1, y: 0 }}
      className="mx-auto mt-6 w-full max-w-md rounded-xl border border-border bg-surface-2 p-6 text-center"
    >
      {auth.status === "starting" ? (
        <div className="flex items-center justify-center gap-2 py-6 text-sm text-content-muted">
          <Loader2 className="size-4 animate-spin" />
          Starting sign-in
        </div>
      ) : (
        <>
          <div className="text-sm text-content-muted">Enter this code to sign in</div>
          <button
            onClick={copy}
            className="group mx-auto mt-3 flex items-center gap-3 rounded-lg border border-border bg-base px-5 py-3"
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
            className="mt-5 w-full"
            onClick={() => auth.verificationUri && openUrl(auth.verificationUri)}
          >
            <ExternalLink className="size-4" />
            Open sign-in page
          </Button>
          <div className="mt-4 flex items-center justify-center gap-2 text-xs text-content-faint">
            <Loader2 className="size-3 animate-spin" />
            Waiting for you to finish in the browser
          </div>
        </>
      )}
    </motion.div>
  );
}

export function AccountsView() {
  const accounts = useStore((s) => s.accounts);
  const auth = useStore((s) => s.auth);
  const addAccount = useStore((s) => s.addAccount);
  const setActiveAccount = useStore((s) => s.setActiveAccount);
  const removeAccount = useStore((s) => s.removeAccount);

  const busy = auth.status === "starting" || auth.status === "pending";

  return (
    <div className="flex flex-1 flex-col">
      <PageHeader
        title="Accounts"
        subtitle="Sign in with your Microsoft account to play online."
        actions={
          <Button onClick={addAccount} disabled={busy}>
            <LogIn className="size-4" />
            Add Microsoft account
          </Button>
        }
      />

      {accounts.length === 0 && auth.status === "idle" ? (
        <EmptyState
          icon={<UserCircle2 className="size-6" />}
          title="No accounts"
          description="Add your Microsoft account to launch Minecraft online."
          action={
            <Button onClick={addAccount}>
              <LogIn className="size-4" />
              Add Microsoft account
            </Button>
          }
        />
      ) : (
        <div className="flex-1 overflow-y-auto p-6">
          <div className="flex flex-col gap-2">
            {accounts.map((acc) => (
              <div
                key={acc.id}
                className={cn(
                  "flex items-center gap-3 rounded-xl border px-4 py-3 transition-colors",
                  acc.active ? "border-lava/40 bg-lava/5" : "border-border bg-surface-2",
                )}
              >
                <PlayerHead uuid={acc.id} name={acc.name} size={40} className="shrink-0 rounded-lg" />
                <div className="min-w-0 flex-1">
                  <div className="truncate font-display font-semibold text-content">
                    {acc.name}
                  </div>
                  <div className="text-xs text-content-muted">
                    {acc.active ? "Active" : "Microsoft account"}
                  </div>
                </div>
                {!acc.active && (
                  <button
                    onClick={() => setActiveAccount(acc.id)}
                    className="rounded-lg border border-border bg-surface-3 px-3 py-1.5 text-xs font-medium text-content hover:bg-border"
                  >
                    Make active
                  </button>
                )}
                <button
                  onClick={() => removeAccount(acc.id)}
                  className="grid size-8 place-items-center rounded-lg text-content-faint transition-colors hover:bg-danger/15 hover:text-danger"
                >
                  <Trash2 className="size-4" />
                </button>
              </div>
            ))}
          </div>

          <AnimatePresence>
            <DeviceCodeCard />
          </AnimatePresence>
        </div>
      )}
    </div>
  );
}
