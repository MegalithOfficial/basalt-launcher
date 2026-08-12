import {
  Check,
  Clock,
  Download,
  Heart,
  Loader2,
  Package,
  Server as ServerIcon,
} from "lucide-react";

import { cn } from "../../lib/cn";
import { relativeTime } from "../../lib/time";
import type { Instance, ProjectDetails, SearchProvider, Server } from "../../lib/types";
import { accentFrom, formatCount } from "../ContentResults";
import { InstanceTargetPicker } from "../InstanceTargetPicker";

function Stat({
  icon: Icon,
  children,
}: {
  icon: typeof Download;
  children: React.ReactNode;
}) {
  return (
    <span className="inline-flex items-center gap-1.5 text-xs text-white/70">
      <Icon className="size-3.5" />
      {children}
    </span>
  );
}

export function ProjectHero({
  details,
  provider,
  loading,
  isPack,
  installedLabel,
  installedNote,
  installing,
  instances,
  target,
  onSelectTarget,
  servers,
  selectedServerId,
  onSelectServer,
  showTargetPicker,
  onInstall,
  onGetServer,
  onOpenInstalled,
}: {
  details: ProjectDetails | null;
  provider: SearchProvider;
  loading: boolean;
  isPack: boolean;
  installedLabel: string | null;
  installedNote?: string | null;
  installing: boolean;
  instances: Instance[];
  target: Instance | null;
  onSelectTarget: (instance: Instance | null) => void;
  servers?: Server[];
  selectedServerId?: string | null;
  onSelectServer?: (serverId: string) => void;
  showTargetPicker: boolean;
  onInstall: () => void;
  onGetServer?: () => void;
  onOpenInstalled: () => void;
}) {
  const backdrop =
    details?.gallery.find((g) => g.featured)?.url ?? details?.gallery[0]?.url ?? null;
  const accent = accentFrom(details?.color ?? null);

  return (
    <div className="relative shrink-0 overflow-hidden border-b border-border-soft">
      {backdrop ? (
        <>
          <img
            src={backdrop}
            className="absolute inset-0 h-full w-full scale-110 object-cover blur-2xl"
            draggable={false}
          />
          <div className="absolute inset-0 bg-linear-to-t from-void via-void/85 to-void/60" />
        </>
      ) : (
        <div
          className="absolute inset-0"
          style={{
            background: accent
              ? `radial-gradient(120% 140% at 12% 0%, ${accent}22, transparent 60%)`
              : undefined,
          }}
        />
      )}

      <div className="relative flex items-start gap-4 px-6 pb-5 pt-12">
        {details?.icon_url ? (
          <img
            src={details.icon_url}
            style={accent ? { boxShadow: `0 0 0 1px ${accent}55, 0 8px 30px ${accent}22` } : undefined}
            className="size-20 shrink-0 rounded-2xl bg-surface-2 object-cover"
            draggable={false}
          />
        ) : (
          <div className="grid size-20 shrink-0 place-items-center rounded-2xl bg-surface-2 text-content-faint">
            <Package className="size-7" />
          </div>
        )}

        <div className="min-w-0 flex-1 pt-0.5">
          <div className="flex min-w-0 items-baseline gap-2.5">
            <h1 className="truncate font-display text-2xl font-bold tracking-tight text-white">
              {details?.title ?? (loading ? "Loading" : "Project")}
            </h1>
            {details?.author && (
              <span className="shrink-0 truncate text-sm font-medium text-white/55">
                by {details.author}
              </span>
            )}
          </div>
          {details?.description && (
            <p className="mt-1 line-clamp-2 max-w-3xl text-sm text-white/70">
              {details.description}
            </p>
          )}
          <div className="mt-2.5 flex flex-wrap items-center gap-x-4 gap-y-1.5">
            {details && <Stat icon={Download}>{formatCount(details.downloads)}</Stat>}
            {!!details?.follows && <Stat icon={Heart}>{formatCount(details.follows)}</Stat>}
            {details?.updated && (
              <Stat icon={Clock}>
                {relativeTime(Math.floor(new Date(details.updated).getTime() / 1000))}
              </Stat>
            )}
            <span className="rounded bg-white/10 px-1.5 py-0.5 text-[10px] uppercase tracking-wide text-white/60">
              {provider}
            </span>
            {details?.license && (
              <span className="text-[11px] text-white/50">{details.license}</span>
            )}
          </div>
        </div>

        <div className="flex shrink-0 flex-col items-end gap-2 pt-1">
          {installedLabel ? (
            <button
              onClick={onOpenInstalled}
              className="inline-flex h-10 items-center gap-2 rounded-xl bg-ok/15 px-5 text-sm font-semibold text-ok transition-colors hover:bg-ok/25"
            >
              <Check className="size-4" />
              {installedLabel}
            </button>
          ) : (
            <button
              onClick={onInstall}
              disabled={installing || loading}
              className="inline-flex h-10 items-center gap-2 rounded-xl px-5 text-sm font-semibold text-black shadow-lg shadow-(color:--accent-glow) transition-all [background:linear-gradient(to_bottom,var(--accent),var(--accent-deep))] hover:[background:linear-gradient(to_bottom,var(--accent-bright),var(--accent))] disabled:opacity-60"
            >
              {installing ? (
                <Loader2 className="size-4 animate-spin" />
              ) : (
                <Download className="size-4" />
              )}
              {isPack ? (installedNote ? "Install again" : "Install pack") : "Install"}
            </button>
          )}

          {onGetServer && (
            <button
              onClick={onGetServer}
              className="inline-flex h-10 items-center gap-2 rounded-xl border border-white/15 bg-white/10 px-4 text-sm font-medium text-white transition-colors hover:bg-white/20"
            >
              <ServerIcon className="size-4" />
              Get server
            </button>
          )}

          {!installedLabel && installedNote && (
            <button
              onClick={onOpenInstalled}
              className="inline-flex items-center gap-1.5 rounded-lg px-2 py-1 text-[11px] font-medium text-white/70 transition-colors hover:bg-white/10 hover:text-white"
            >
              <Check className="size-3 text-ok" />
              {installedNote}
            </button>
          )}

          {showTargetPicker && (
            <InstanceTargetPicker
              instances={instances}
              selected={target}
              onSelect={onSelectTarget}
              servers={servers}
              selectedServerId={selectedServerId}
              onSelectServer={onSelectServer}
            />
          )}
        </div>
      </div>

      {!!details?.categories.length && (
        <div className="relative flex flex-wrap gap-1.5 px-6 pb-4">
          {details.categories.map((category) => (
            <span
              key={category}
              className={cn(
                "rounded-md bg-white/[0.07] px-2 py-0.5 text-[11px] font-medium capitalize text-white/70",
              )}
            >
              {category}
            </span>
          ))}
        </div>
      )}
    </div>
  );
}
