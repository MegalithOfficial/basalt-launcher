import { Area, AreaChart, CartesianGrid, ResponsiveContainer, Tooltip, YAxis } from "recharts";

import type { ServerUsage } from "../../lib/types";

const GRID = "var(--color-border-soft)";
const TICK = { fill: "var(--color-content-faint)", fontSize: 10 };

function ValueTooltip({
  active,
  payload,
  format,
  secondsPerSample,
}: {
  active?: boolean;
  payload?: Array<{ payload: { value: number; index: number } }>;
  format: (value: number) => string;
  secondsPerSample: number;
}) {
  if (!active || !payload?.length) return null;
  const { value, index } = payload[0].payload;
  const ago = Math.round(index * secondsPerSample);
  return (
    <div className="rounded-lg border border-border bg-surface-3 px-2.5 py-1.5 shadow-xl">
      <div className="font-mono text-[11px] tabular-nums text-content">{format(value)}</div>
      <div className="mt-0.5 text-[10px] text-content-muted">{ago}s into this window</div>
    </div>
  );
}

export function UsageChart({
  samples,
  pick,
  ceiling,
  tone,
  height,
  format,
  axisFormat,
  axisWidth = 40,
  secondsPerSample = 2,
}: {
  samples: ServerUsage[];
  pick: (sample: ServerUsage) => number;
  ceiling: number;
  tone: string;
  height: number;
  format: (value: number) => string;
  axisFormat?: (value: number) => string;
  axisWidth?: number;
  secondsPerSample?: number;
}) {
  if (samples.length < 2) {
    return (
      <div className="flex items-end" style={{ height }}>
        <div className="h-px w-full bg-border-soft" />
      </div>
    );
  }

  const data = samples.map((sample, index) => ({ index, value: pick(sample) }));
  return (
    <div style={{ height }}>
      <ResponsiveContainer width="100%" height="100%">
        <AreaChart data={data} margin={{ top: 8, right: 0, bottom: 0, left: 0 }}>
          <CartesianGrid vertical={false} stroke={GRID} strokeDasharray="0" strokeWidth={1} />
          <YAxis
            orientation="right"
            domain={[0, ceiling]}
            tickFormatter={(value) => (axisFormat ?? format)(Number(value))}
            tick={TICK}
            tickLine={false}
            axisLine={false}
            width={axisWidth}
            tickCount={3}
          />
          <Tooltip
            cursor={{ stroke: GRID, strokeWidth: 1 }}
            content={
              <ValueTooltip format={format} secondsPerSample={secondsPerSample} />
            }
          />
          <Area
            type="monotone"
            dataKey="value"
            stroke={tone}
            strokeWidth={1.5}
            fill={tone}
            fillOpacity={0.12}
            isAnimationActive={false}
            dot={false}
            activeDot={{ r: 2.5, strokeWidth: 0, fill: tone }}
          />
        </AreaChart>
      </ResponsiveContainer>
    </div>
  );
}
