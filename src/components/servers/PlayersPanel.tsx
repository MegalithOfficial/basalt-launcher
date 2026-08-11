import { useCallback, useEffect, useState } from "react";
import { Loader2, Plus, ShieldBan, ShieldCheck, Trash2, UserCheck } from "lucide-react";

import { api } from "../../lib/api";
import { cn } from "../../lib/cn";
import { Toggle } from "../ui";
import type { PlayerEntry, PlayerList, Server } from "../../lib/types";

const LISTS: Array<{
  id: PlayerList;
  label: string;
  icon: typeof UserCheck;
  file: string;
  empty: string;
}> = [
  {
    id: "ops",
    label: "Operators",
    icon: ShieldCheck,
    file: "ops.json",
    empty: "Nobody can run commands yet.",
  },
  {
    id: "whitelist",
    label: "Whitelist",
    icon: UserCheck,
    file: "whitelist.json",
    empty: "Nobody is whitelisted. Turn white-list on in Properties to enforce it.",
  },
  {
    id: "banned",
    label: "Banned",
    icon: ShieldBan,
    file: "banned-players.json",
    empty: "Nobody is banned.",
  },
];

const SAVE_DELAY_MS = 600;

export function PlayersPanel({ server, live }: { server: Server; live: boolean }) {
  const [list, setList] = useState<PlayerList>("ops");
  const [entries, setEntries] = useState<PlayerEntry[]>([]);
  const [name, setName] = useState("");
  const [reason, setReason] = useState("");
  const [enforced, setEnforced] = useState<boolean | null>(null);
  const [loading, setLoading] = useState(true);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const active = LISTS.find((entry) => entry.id === list)!;

  const load = useCallback(async () => {
    setLoading(true);
    try {
      setEntries(await api.listServerPlayers(server.id, list));
      if (list === "whitelist") {
        const properties = await api.getServerProperties(server.id);
        const flag = properties.find((entry) => entry.key === "white-list")?.value;
        setEnforced(flag?.trim().toLowerCase() === "true");
      }
      setError(null);
    } catch (cause) {
      setError(String(cause));
    } finally {
      setLoading(false);
    }
  }, [server.id, list]);

  useEffect(() => {
    void load();
  }, [load]);

  const run = async (action: () => Promise<unknown>) => {
    setBusy(true);
    setError(null);
    try {
      await action();
      if (live) await new Promise((resolve) => setTimeout(resolve, SAVE_DELAY_MS));
      await load();
    } catch (cause) {
      setError(String(cause));
    } finally {
      setBusy(false);
    }
  };

  const add = async () => {
    const player = name.trim();
    if (!player) return;
    await run(async () => {
      await api.addServerPlayer(server.id, list, player, reason.trim() || null);
      setName("");
      setReason("");
    });
  };

  return (
    <div className="flex min-h-0 flex-1 flex-col">
      <div className="flex flex-wrap items-center gap-3 px-6 pb-1 pt-4">
        <div
          role="group"
          aria-label="Player list"
          className="flex shrink-0 items-center gap-0.5 rounded-lg border border-border-soft bg-surface-2/60 p-0.5"
        >
          {LISTS.map((option) => (
            <button
              key={option.id}
              onClick={() => setList(option.id)}
              className={cn(
                "flex items-center gap-1.5 rounded-md px-2.5 py-1 text-xs font-medium transition-colors",
                list === option.id
                  ? "bg-surface-3 text-content"
                  : "text-content-faint hover:text-content-muted",
              )}
            >
              <option.icon className="size-3.5" />
              {option.label}
            </button>
          ))}
        </div>

        <span className="font-pixel text-[10px] uppercase tracking-[0.28em] text-content-faint">
          {entries.length} in {active.file}
        </span>

        {list === "whitelist" && enforced !== null && (
          <div className="flex items-center gap-2">
            <Toggle
              label="Enforce the whitelist"
              checked={enforced}
              disabled={busy}
              onChange={(next) => void run(() => api.setServerWhitelist(server.id, next))}
            />
            <span
              className={cn(
                "text-[12px]",
                enforced ? "text-content" : "text-content-faint",
              )}
            >
              {enforced ? "Enforced" : "Off, anyone can join"}
            </span>
          </div>
        )}

        {live && (
          <span className="ml-auto font-pixel text-[10px] uppercase tracking-[0.22em] text-content-faint">
            run as console commands
          </span>
        )}
      </div>

      <div className="flex flex-wrap items-center gap-2 px-6 pt-3">
        <input
          value={name}
          onChange={(event) => setName(event.target.value)}
          onKeyDown={(event) => event.key === "Enter" && void add()}
          placeholder="Player name"
          className="w-56 rounded-lg border border-border bg-void px-3 py-2 text-sm text-content outline-none transition-colors focus:border-(--accent)"
        />
        {list === "banned" && (
          <input
            value={reason}
            onChange={(event) => setReason(event.target.value)}
            onKeyDown={(event) => event.key === "Enter" && void add()}
            placeholder="Reason, optional"
            className="w-72 rounded-lg border border-border bg-void px-3 py-2 text-sm text-content outline-none transition-colors focus:border-(--accent)"
          />
        )}
        <button
          onClick={() => void add()}
          disabled={busy || !name.trim()}
          className="inline-flex items-center gap-1.5 rounded-lg px-3 py-2 text-xs font-semibold text-black shadow-md shadow-(color:--accent-glow) transition-all [background:linear-gradient(to_bottom,var(--accent),var(--accent-deep))] hover:[background:linear-gradient(to_bottom,var(--accent-bright),var(--accent))] disabled:cursor-not-allowed disabled:opacity-40 disabled:shadow-none"
        >
          {busy ? <Loader2 className="size-3.5 animate-spin" /> : <Plus className="size-3.5" />}
          {list === "banned" ? "Ban" : "Add"}
        </button>
      </div>

      {error && (
        <div className="mx-6 mt-3 wrap-break-word rounded-lg border border-danger/30 bg-danger/10 px-3 py-2 text-[11px] text-danger">
          {error}
        </div>
      )}

      <div className="min-h-0 flex-1 overflow-y-auto px-6 py-4">
        {loading ? (
          <div className="flex items-center justify-center gap-2 py-12 text-sm text-content-muted">
            <Loader2 className="size-4 animate-spin" />
            Loading
          </div>
        ) : entries.length === 0 ? (
          <div className="py-16 text-center text-sm text-content-faint">
            {active.empty}
          </div>
        ) : (
          <div className="flex flex-col gap-1.5">
            {entries.map((entry) => (
              <div
                key={entry.uuid || entry.name}
                className="flex items-center gap-3 rounded-xl border border-border-soft bg-surface-2/70 px-4 py-2.5"
              >
                <div className="min-w-0 flex-1">
                  <div className="flex items-center gap-2">
                    <span className="truncate text-sm font-medium text-content">{entry.name}</span>
                    {entry.level !== null && entry.level !== undefined && (
                      <span className="shrink-0 rounded bg-surface-3 px-1.5 py-0.5 text-[9px] font-semibold uppercase tracking-wide text-content-faint">
                        level {entry.level}
                      </span>
                    )}
                  </div>
                  <div className="truncate font-mono text-[11px] text-content-faint">
                    {entry.uuid}
                    {entry.reason && ` · ${entry.reason}`}
                  </div>
                </div>
                <button
                  onClick={() =>
                    void run(() => api.removeServerPlayer(server.id, list, entry.name || entry.uuid))
                  }
                  disabled={busy}
                  aria-label={list === "banned" ? "Pardon" : "Remove"}
                  title={list === "banned" ? "Pardon" : "Remove"}
                  className="grid size-8 place-items-center rounded-lg text-content-faint transition-colors hover:bg-danger/15 hover:text-danger disabled:opacity-40"
                >
                  <Trash2 className="size-4" />
                </button>
              </div>
            ))}
          </div>
        )}
      </div>
    </div>
  );
}
