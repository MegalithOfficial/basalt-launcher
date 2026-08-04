import { useCallback, useEffect, useMemo, useState } from "react";
import { ChartNoAxesColumn, TriangleAlert } from "lucide-react";
import {
  Bar,
  BarChart,
  CartesianGrid,
  ResponsiveContainer,
  Tooltip,
  XAxis,
  YAxis,
} from "recharts";

import { api } from "../lib/api";
import { cn } from "../lib/cn";
import { log } from "../lib/log";
import { formatDuration, relativeTime } from "../lib/time";
import type { PlayStats } from "../lib/types";
import { EmptyState } from "../components/ui";
import { useStore } from "../store";

const RANGES: Array<{ label: string; days: number | null }> = [
  { label: "7 days", days: 7 },
  { label: "30 days", days: 30 },
  { label: "90 days", days: 90 },
  { label: "1 year", days: 365 },
  { label: "All", days: null },
];

const WEEKDAYS = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];
const MINECRAFT_DAY_SECS = 1200;
const DAILY_CHART_MIN_DAYS = 2;
const RHYTHM_MIN_DAYS = 3;

const GRID = "var(--color-border-soft)";
const TICK = { fill: "var(--color-content-faint)", fontSize: 11 };
const Y_DOMAIN: [number, (dataMax: number) => number] = [
  0,
  (dataMax) => Math.max(dataMax, 3600),
];

function hoursTick(secs: number): string {
  if (secs <= 0) return "0";
  const hours = secs / 3600;
  if (hours < 1) return `${Math.round(secs / 60)}m`;
  return `${hours < 10 ? Math.round(hours * 10) / 10 : Math.round(hours)}h`;
}

function dayTick(date: string): string {
  const parsed = new Date(`${date}T00:00:00`);
  return parsed.toLocaleDateString(undefined, { month: "short", day: "numeric" });
}

function durationParts(secs: number): Array<{ value: string; unit: string }> {
  if (secs < 60) return [{ value: String(Math.max(0, secs)), unit: "s" }];
  const hours = Math.floor(secs / 3600);
  const minutes = Math.floor((secs % 3600) / 60);
  if (hours === 0) return [{ value: String(minutes), unit: "m" }];
  const parts = [{ value: String(hours), unit: "h" }];
  if (minutes > 0) parts.push({ value: String(minutes), unit: "m" });
  return parts;
}

type TickFormatter = (value: string | number) => string;
type RechartsTickFormatter = React.ComponentProps<typeof XAxis>["tickFormatter"];

function SectionLabel({ children }: { children: React.ReactNode }) {
  return (
    <h2 className="font-pixel text-[10px] uppercase tracking-[0.28em] text-content-faint">
      {children}
    </h2>
  );
}

function ChartTooltip({
  active,
  payload,
  title,
}: {
  active?: boolean;
  payload?: Array<{ payload: Record<string, unknown> }>;
  title: (row: Record<string, unknown>) => string;
}) {
  if (!active || !payload?.length) return null;
  const row = payload[0].payload;
  const secs = Number(row.secs ?? 0);
  const sessions = Number(row.sessions ?? 0);
  return (
    <div className="rounded-lg border border-border bg-surface-3 px-2.5 py-1.5 shadow-xl">
      <div className="text-[11px] font-medium text-content">{title(row)}</div>
      <div className="mt-0.5 text-[11px] tabular-nums text-content-muted">
        {formatDuration(secs)}
        {sessions > 0 && ` · ${sessions} session${sessions === 1 ? "" : "s"}`}
      </div>
    </div>
  );
}

function PlaytimeChart({
  data,
  dataKey,
  tickFormatter,
  tooltipTitle,
  interval,
  maxBarSize,
  height,
}: {
  data: Array<Record<string, unknown>>;
  dataKey: string;
  tickFormatter?: TickFormatter;
  tooltipTitle: (row: Record<string, unknown>) => string;
  interval?: number | "preserveStartEnd";
  maxBarSize: number;
  height: number;
}) {
  return (
    <div style={{ height }} className="w-full">
      <ResponsiveContainer width="100%" height="100%">
        <BarChart data={data} margin={{ top: 4, right: 4, bottom: 0, left: 0 }}>
          <CartesianGrid vertical={false} stroke={GRID} strokeDasharray="0" strokeWidth={1} />
          <XAxis
            dataKey={dataKey}
            tickFormatter={tickFormatter as RechartsTickFormatter}
            tick={TICK}
            tickLine={false}
            axisLine={false}
            minTickGap={28}
            interval={interval}
          />
          <YAxis
            tickFormatter={hoursTick}
            tick={TICK}
            tickLine={false}
            axisLine={false}
            width={44}
            domain={Y_DOMAIN}
          />
          <Tooltip
            cursor={{ fill: "var(--color-surface-2)" }}
            content={<ChartTooltip title={tooltipTitle} />}
          />
          <Bar
            dataKey="secs"
            fill="var(--accent)"
            radius={[4, 4, 0, 0]}
            maxBarSize={maxBarSize}
            isAnimationActive={false}
          />
        </BarChart>
      </ResponsiveContainer>
    </div>
  );
}

function StatsHeader({
  subtitle,
  children,
}: {
  subtitle: string;
  children?: React.ReactNode;
}) {
  return (
    <div className="flex flex-wrap items-center justify-between gap-x-4 gap-y-2 border-b border-border-soft px-8 py-3.5">
      <div className="flex min-w-0 items-baseline gap-2.5">
        <h1 className="font-display text-base font-semibold tracking-tight text-content">
          Stats
        </h1>
        <span className="text-[11px] text-content-muted">{subtitle}</span>
      </div>
      {children}
    </div>
  );
}

export function StatsView() {
  const [days, setDays] = useState<number | null>(30);
  const [stats, setStats] = useState<PlayStats | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const openInstance = useStore((s) => s.openInstance);

  const load = useCallback(async (window: number | null) => {
    setLoading(true);
    try {
      const result = await api.getPlayStats(window);
      setStats(result);
      setError(null);
    } catch (cause) {
      log.error("stats", `could not load play stats: ${String(cause)}`);
      setError(String(cause));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void load(days);
  }, [days, load]);

  const hourly = useMemo(
    () =>
      (stats?.hourly ?? []).map((secs, hour) => ({
        hour,
        label: `${String(hour).padStart(2, "0")}:00`,
        secs,
      })),
    [stats],
  );

  const weekday = useMemo(
    () =>
      (stats?.weekday ?? []).map((secs, index) => ({
        label: WEEKDAYS[index],
        secs,
      })),
    [stats],
  );

  if (error) {
    return (
      <>
        <StatsHeader subtitle="Could not read your session history" />
        <EmptyState
          icon={<TriangleAlert className="size-6" />}
          title="Could not load stats"
          description={error}
        />
      </>
    );
  }

  if (!stats && loading) {
    return (
      <>
        <StatsHeader subtitle="Loading" />
        <div className="flex-1" />
      </>
    );
  }

  if (!stats) return null;

  if (stats.tracked_since === null) {
    return (
      <>
        <StatsHeader subtitle="Session history starts with your next launch" />
        <EmptyState
          icon={<ChartNoAxesColumn className="size-6" />}
          title={stats.lifetime_secs > 0 ? "No session history yet" : "No playtime yet"}
          description={
            stats.lifetime_secs > 0
              ? `Basalt has ${formatDuration(stats.lifetime_secs)} of playtime on record from before it tracked individual sessions. The breakdowns start filling in with your next launch.`
              : "Launch an instance and your sessions will start showing up here."
          }
        />
      </>
    );
  }

  const rangeLabel = RANGES.find((range) => range.days === days)?.label ?? "all time";
  const hero = durationParts(stats.window_secs);
  const minecraftDays = Math.round(stats.window_secs / MINECRAFT_DAY_SECS);
  const topLifetimeSecs = Math.max(...stats.instances.map((row) => row.lifetime_secs), 0);
  const showDaily = stats.active_days >= DAILY_CHART_MIN_DAYS;
  const showRhythm = stats.active_days >= RHYTHM_MIN_DAYS;
  const showShare = stats.instances.length > 1;

  const facts: Array<{ value: string; label: string }> = [
    {
      value: String(stats.session_count),
      label: stats.session_count === 1 ? "session" : "sessions",
    },
  ];
  if (stats.session_count > 1) {
    facts.push({ value: formatDuration(stats.average_session_secs), label: "average" });
    facts.push({ value: formatDuration(stats.longest_session_secs), label: "longest" });
  }
  if (stats.active_days > 1) {
    facts.push({ value: String(stats.active_days), label: "active days" });
  }
  if (stats.current_streak_days > 1) {
    facts.push({ value: `${stats.current_streak_days}d`, label: "streak" });
  }

  return (
    <div className="flex min-h-0 flex-1 flex-col">
      <StatsHeader subtitle={`Since ${relativeTime(stats.tracked_since)}`}>
        <div className="flex flex-wrap items-center gap-1.5">
          {RANGES.map((range) => (
            <button
              key={range.label}
              type="button"
              onClick={() => setDays(range.days)}
              className={cn(
                "rounded-full border px-2.5 py-0.5 text-[11px] font-medium transition-colors",
                range.days === days
                  ? "border-(--accent)/50 bg-(--accent)/12 text-(--accent)"
                  : "border-border-soft bg-surface-2 text-content-muted hover:border-border hover:text-content",
              )}
            >
              {range.label}
            </button>
          ))}
        </div>
      </StatsHeader>

      <div className="min-h-0 flex-1 overflow-y-auto">
        <div
          className={cn(
            "px-8 pb-16 pt-8 transition-opacity duration-150",
            loading && "opacity-50",
          )}
        >
          <div className="font-pixel text-[10px] uppercase tracking-[0.28em] text-content-faint">
            Played · {rangeLabel}
          </div>
          <div className="mt-3 flex items-baseline gap-3 font-display text-6xl font-semibold leading-none tracking-tight text-content">
            {hero.map((part) => (
              <span key={part.unit} className="flex items-baseline">
                {part.value}
                <span className="ml-1 text-2xl font-medium text-content-muted">
                  {part.unit}
                </span>
              </span>
            ))}
          </div>
          {minecraftDays >= 1 && (
            <div className="mt-3.5 font-pixel text-[11px] tracking-wider text-(--accent)">
              {minecraftDays.toLocaleString()} Minecraft{" "}
              {minecraftDays === 1 ? "day" : "days"}
            </div>
          )}

          <div className="mt-5 flex flex-wrap items-baseline gap-x-5 gap-y-1.5 text-[13px]">
            {facts.map((fact) => (
              <span key={fact.label}>
                <span className="tabular-nums text-content">{fact.value}</span>{" "}
                <span className="text-content-faint">{fact.label}</span>
              </span>
            ))}
            {stats.crash_count > 0 && (
              <span>
                <span className="tabular-nums text-danger">{stats.crash_count}</span>{" "}
                <span className="text-content-faint">
                  {stats.crash_count === 1 ? "crash" : "crashes"}
                </span>
              </span>
            )}
            <span className="text-content-faint">
              {formatDuration(stats.lifetime_secs)} all time
            </span>
          </div>

          {showDaily && (
            <section className="mt-12">
              <SectionLabel>Per day</SectionLabel>
              <div className="mt-4">
                <PlaytimeChart
                  data={stats.daily as unknown as Array<Record<string, unknown>>}
                  dataKey="date"
                  tickFormatter={(value) => dayTick(String(value))}
                  tooltipTitle={(row) => dayTick(String(row.date))}
                  interval="preserveStartEnd"
                  maxBarSize={26}
                  height={220}
                />
              </div>
            </section>
          )}

          {stats.instances.length > 0 && (
            <section className="mt-12">
              <SectionLabel>Instances · all time</SectionLabel>
              <table className="mt-4 w-full text-left text-[13px]">
                <thead className="sr-only">
                  <tr>
                    <th>Instance</th>
                    {showShare && <th>Share of playtime</th>}
                    <th>Played all time</th>
                    <th>Played in range</th>
                    <th>Last played</th>
                  </tr>
                </thead>
                <tbody>
                  {stats.instances.map((row) => (
                    <tr
                      key={row.instance_id}
                      onClick={() => !row.deleted && openInstance(row.instance_id)}
                      className={cn(
                        "border-b border-border-soft last:border-0",
                        !row.deleted && "cursor-pointer hover:bg-surface/60",
                      )}
                    >
                      <td className="py-2.5 pr-4 align-middle">
                        <div className="flex items-center gap-2">
                          <span className="font-medium text-content">{row.name}</span>
                          {row.deleted && (
                            <span className="shrink-0 rounded bg-surface-2 px-1.5 py-0.5 font-pixel text-[9px] tracking-wider text-content-faint">
                              deleted
                            </span>
                          )}
                        </div>
                      </td>
                      {showShare && (
                        <td className="w-[38%] py-2.5 pl-6 pr-8 align-middle">
                          <div className="h-1.5 w-full overflow-hidden rounded-full bg-surface-2">
                            <div
                              className="h-full rounded-full bg-(--accent)"
                              style={{
                                width: `${topLifetimeSecs > 0 ? (row.lifetime_secs / topLifetimeSecs) * 100 : 0}%`,
                              }}
                            />
                          </div>
                        </td>
                      )}
                      <td className="w-px whitespace-nowrap py-2.5 text-right align-middle tabular-nums text-content">
                        {formatDuration(row.lifetime_secs)}
                      </td>
                      <td className="w-px whitespace-nowrap py-2.5 pl-4 text-right align-middle tabular-nums text-content-faint">
                        {row.secs > 0 ? `+${formatDuration(row.secs)}` : ""}
                      </td>
                      <td className="w-px whitespace-nowrap py-2.5 pl-8 text-right align-middle text-content-faint">
                        {row.last_played_at === null
                          ? "never"
                          : relativeTime(row.last_played_at)}
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </section>
          )}

          {showRhythm && (
            <section className="mt-12">
              <SectionLabel>When you play</SectionLabel>
              <div className="mt-4 grid grid-cols-1 gap-8 lg:grid-cols-2">
                <PlaytimeChart
                  data={hourly}
                  dataKey="hour"
                  tickFormatter={(value) => String(value).padStart(2, "0")}
                  tooltipTitle={(row) => String(row.label)}
                  interval={2}
                  maxBarSize={16}
                  height={170}
                />
                <PlaytimeChart
                  data={weekday}
                  dataKey="label"
                  tooltipTitle={(row) => String(row.label)}
                  maxBarSize={34}
                  height={170}
                />
              </div>
            </section>
          )}

          {stats.recent.length > 0 && (
            <section className="mt-12">
              <SectionLabel>Recent sessions</SectionLabel>
              <table className="mt-4 w-full text-left text-[13px]">
                <thead className="sr-only">
                  <tr>
                    <th>Instance</th>
                    <th>Version</th>
                    <th>Started</th>
                    <th>Duration</th>
                  </tr>
                </thead>
                <tbody>
                  {stats.recent.map((session) => (
                    <tr key={session.id} className="border-b border-border-soft last:border-0">
                      <td className="py-2.5 pr-4 align-middle">
                        <div className="flex items-center gap-2">
                          <span className="font-medium text-content">
                            {session.instance_name}
                          </span>
                          {session.crashed && (
                            <span className="shrink-0 rounded bg-danger/15 px-1.5 py-0.5 font-pixel text-[9px] tracking-wider text-danger">
                              crashed
                            </span>
                          )}
                        </div>
                      </td>
                      <td className="w-px whitespace-nowrap py-2.5 pl-4 align-middle font-pixel text-[10px] tracking-wider text-content-faint">
                        {session.version_id || "unknown"}
                        {session.loader && ` · ${session.loader}`}
                      </td>
                      <td className="w-px whitespace-nowrap py-2.5 pl-8 text-right align-middle text-content-faint">
                        {new Date(session.started_at * 1000).toLocaleString(undefined, {
                          month: "short",
                          day: "numeric",
                          hour: "2-digit",
                          minute: "2-digit",
                        })}
                      </td>
                      <td className="w-px whitespace-nowrap py-2.5 pl-8 text-right align-middle tabular-nums text-content">
                        {formatDuration(session.played_secs)}
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </section>
          )}

          {!showDaily && (
            <p className="mt-12 max-w-md text-[13px] text-content-faint">
              Play on a few more days and the per day, per instance and per hour breakdowns
              open up here.
            </p>
          )}
        </div>
      </div>
    </div>
  );
}
