import { LogIn, Trash2, UserCircle2 } from "lucide-react";

import { cn } from "../lib/cn";
import { Button, EmptyState } from "../components/ui";
import { PlayerHead } from "../components/Avatar";
import { AppearancePanel } from "../components/AppearancePanel";
import { SignInModal } from "../components/SignInModal";
import { useStore } from "../store";

function AccountCard({
  id,
  name,
  active,
  onActivate,
  onRemove,
}: {
  id: string;
  name: string;
  active: boolean;
  onActivate: () => void;
  onRemove: () => void;
}) {
  return (
    <div
      className={cn(
        "flex flex-col gap-4 rounded-2xl border p-4 transition-colors",
        active
          ? "border-[var(--accent)]/45 bg-[var(--accent-glow)]/25"
          : "border-border-soft bg-surface-2/60 hover:border-border",
      )}
    >
      <div className="flex items-start gap-3">
        <PlayerHead
          uuid={id}
          name={name}
          size={52}
          className={cn(
            "shrink-0 rounded-xl",
            active && "shadow-lg shadow-[var(--accent-glow)]",
          )}
        />
        <div className="min-w-0 flex-1">
          <div className="truncate font-display text-base font-semibold text-content">
            {name}
          </div>
          {active ? (
            <span className="mt-1.5 inline-block rounded-md border border-[var(--accent)]/40 px-1.5 py-0.5 text-[10px] font-semibold uppercase tracking-wide text-[var(--accent-bright)]">
              Active
            </span>
          ) : (
            <div className="mt-1 text-xs text-content-faint">Microsoft account</div>
          )}
        </div>
      </div>

      <div className="mt-auto flex items-center gap-2">
        {active ? (
          <span className="flex-1 text-xs text-content-faint">Used for every launch</span>
        ) : (
          <button
            onClick={onActivate}
            className="flex-1 rounded-lg border border-border bg-surface-2 px-3 py-2 text-xs font-medium text-content transition-colors hover:bg-surface-3"
          >
            Make active
          </button>
        )}
        <button
          onClick={onRemove}
          title={`Remove ${name}`}
          className="grid size-8 shrink-0 place-items-center rounded-lg text-content-faint transition-colors hover:bg-danger/15 hover:text-danger"
        >
          <Trash2 className="size-4" />
        </button>
      </div>
    </div>
  );
}

export function AccountsView() {
  const accounts = useStore((s) => s.accounts);
  const auth = useStore((s) => s.auth);
  const addAccount = useStore((s) => s.addAccount);
  const setActiveAccount = useStore((s) => s.setActiveAccount);
  const removeAccount = useStore((s) => s.removeAccount);

  const busy = auth.status === "starting" || auth.status === "pending";
  const active = accounts.find((a) => a.active) ?? null;

  return (
    <div className="flex min-h-0 flex-1 flex-col">
      <div className="flex items-center justify-between gap-4 border-b border-border-soft px-8 py-3.5">
        <div className="flex items-baseline gap-3">
          <h1 className="font-display text-base font-semibold tracking-tight text-content">
            Accounts
          </h1>
          {accounts.length > 0 && (
            <span className="text-xs text-content-faint">{accounts.length} signed in</span>
          )}
        </div>
        <Button onClick={addAccount} disabled={busy}>
          <LogIn className="size-4" />
          Add Microsoft account
        </Button>
      </div>

      {accounts.length === 0 ? (
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
        <div className="min-h-0 flex-1 overflow-y-auto px-8 py-6">
          <div className="grid gap-3 [grid-template-columns:repeat(auto-fill,minmax(17rem,1fr))]">
            {accounts.map((acc) => (
              <AccountCard
                key={acc.id}
                id={acc.id}
                name={acc.name}
                active={acc.active}
                onActivate={() => setActiveAccount(acc.id)}
                onRemove={() => removeAccount(acc.id)}
              />
            ))}
          </div>

          {active && (
            <div className="mt-6">
              <AppearancePanel key={active.id} accountName={active.name} />
            </div>
          )}
        </div>
      )}

      <SignInModal />
    </div>
  );
}
