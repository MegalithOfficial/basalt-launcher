import { openUrl } from "@tauri-apps/plugin-opener";
import { ExternalLink } from "lucide-react";

import { cn } from "../../lib/cn";
import { relativeTime } from "../../lib/time";
import type { ProjectDetails } from "../../lib/types";

function SideCard({ title, children }: { title: string; children: React.ReactNode }) {
  return (
    <div className="rounded-2xl border border-border-soft bg-surface-2/60 p-4">
      <div className="mb-2.5 text-sm font-semibold text-content">{title}</div>
      {children}
    </div>
  );
}

function Chip({
  children,
  tone = "default",
}: {
  children: React.ReactNode;
  tone?: "default" | "accent" | "ok";
}) {
  return (
    <span
      className={cn(
        "rounded-md px-2 py-0.5 text-[11px] font-medium",
        tone === "accent" && "bg-(--accent-glow) text-content",
        tone === "ok" && "bg-ok/20 text-ok",
        tone === "default" && "bg-surface-3 text-content-muted",
      )}
    >
      {children}
    </span>
  );
}

function environmentLabel(clientSide: string | null, serverSide: string | null): string | null {
  if (!clientSide && !serverSide) return null;
  if (clientSide === "required" && serverSide === "unsupported") return "Client-side";
  if (serverSide === "required" && clientSide === "unsupported") return "Server-side";
  return "Client and server";
}

export function ProjectSidebar({
  details,
  instanceVersion,
  instanceLoader,
}: {
  details: ProjectDetails;
  instanceVersion: string | null;
  instanceLoader: string | null;
}) {
  const environment = environmentLabel(details.client_side, details.server_side);
  const versions = details.game_versions.slice(0, 14);
  const more = details.game_versions.length - versions.length;

  return (
    <aside className="flex w-64 shrink-0 flex-col gap-3">
      {(versions.length > 0 || details.loaders.length > 0) && (
        <SideCard title="Compatibility">
          {versions.length > 0 && (
            <div className="flex flex-wrap gap-1.5">
              {versions.map((v) => (
                <Chip key={v} tone={v === instanceVersion ? "ok" : "default"}>
                  {v}
                </Chip>
              ))}
              {more > 0 && <Chip>+{more}</Chip>}
            </div>
          )}
          {details.loaders.length > 0 && (
            <>
              <div className="mb-1.5 mt-3 text-xs text-content-faint">Platforms</div>
              <div className="flex flex-wrap gap-1.5">
                {details.loaders.map((l) => (
                  <Chip key={l} tone={l === instanceLoader ? "ok" : "accent"}>
                    {l}
                  </Chip>
                ))}
              </div>
            </>
          )}
          {environment && (
            <>
              <div className="mb-1.5 mt-3 text-xs text-content-faint">Environment</div>
              <Chip>{environment}</Chip>
            </>
          )}
        </SideCard>
      )}

      {details.links.length > 0 && (
        <SideCard title="Links">
          <div className="flex flex-col gap-1">
            {details.links.map((link) => (
              <button
                key={link.url}
                onClick={() => openUrl(link.url)}
                className="flex items-center justify-between rounded-lg px-2 py-1.5 text-left text-xs font-medium text-content-muted transition-colors hover:bg-surface-3 hover:text-content"
              >
                {link.label}
                <ExternalLink className="size-3 text-content-faint" />
              </button>
            ))}
          </div>
        </SideCard>
      )}

      <SideCard title="Details">
        <div className="flex flex-col gap-1.5 text-xs text-content-muted">
          {details.license && <div>Licensed {details.license}</div>}
          {details.published && (
            <div>
              Published {relativeTime(Math.floor(new Date(details.published).getTime() / 1000))}
            </div>
          )}
          {details.updated && (
            <div>
              Updated {relativeTime(Math.floor(new Date(details.updated).getTime() / 1000))}
            </div>
          )}
        </div>
      </SideCard>
    </aside>
  );
}
