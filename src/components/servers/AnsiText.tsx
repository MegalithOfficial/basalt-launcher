import { cn } from "../../lib/cn";
import type { AnsiSpan } from "../../lib/ansi";

export function AnsiText({ spans }: { spans: AnsiSpan[] }) {
  return (
    <>
      {spans.map((span, index) => (
        <span
          key={index}
          style={{ color: span.color, backgroundColor: span.background }}
          className={cn(
            span.bold && "font-semibold",
            span.dim && "opacity-65",
            span.italic && "italic",
            span.underline && "underline",
          )}
        >
          {span.text}
        </span>
      ))}
    </>
  );
}
