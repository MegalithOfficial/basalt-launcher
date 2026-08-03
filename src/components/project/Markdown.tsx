import { memo, useMemo } from "react";
import type { MouseEvent } from "react";
import { openUrl } from "@tauri-apps/plugin-opener";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import rehypeRaw from "rehype-raw";
import rehypeSanitize, { defaultSchema } from "rehype-sanitize";
import DOMPurify from "dompurify";

import { cn } from "../../lib/cn";

const sanitizeSchema = {
  ...defaultSchema,
  tagNames: [...(defaultSchema.tagNames ?? []), "center", "details", "summary"],
  attributes: {
    ...defaultSchema.attributes,
    img: [...(defaultSchema.attributes?.img ?? []), "width", "height", "align"],
    "*": [...(defaultSchema.attributes?.["*"] ?? []), "align"],
  },
};

interface MarkdownProps {
  body: string;
  format: "markdown" | "html";
  className?: string;
}

function openExternalLink(event: MouseEvent<HTMLDivElement>) {
  if (!(event.target instanceof Element)) return;
  const anchor = event.target.closest("a[href]");
  if (!(anchor instanceof HTMLAnchorElement)) return;

  let href = anchor.getAttribute("href")?.trim();
  if (!href || href.startsWith("#")) return;
  if (href.startsWith("//")) href = `https:${href}`;

  try {
    const url = new URL(href);
    if (url.protocol !== "http:" && url.protocol !== "https:") return;
  } catch {
    return;
  }

  event.preventDefault();
  void openUrl(href);
}

export function Markdown(props: MarkdownProps) {
  return <MemoizedMarkdown {...props} />;
}

const MemoizedMarkdown = memo(function MemoizedMarkdown({
  body,
  format,
  className,
}: MarkdownProps) {
  const sanitized = useMemo(
    () => (format === "html" ? DOMPurify.sanitize(body) : ""),
    [body, format],
  );

  if (format === "markdown") {
    return (
      <div className={cn("prose-basalt", className)} onClick={openExternalLink}>
        <ReactMarkdown
          remarkPlugins={[remarkGfm]}
          rehypePlugins={[rehypeRaw, [rehypeSanitize, sanitizeSchema]]}
        >
          {body}
        </ReactMarkdown>
      </div>
    );
  }
  return (
    <div
      className={cn("prose-basalt", className)}
      onClick={openExternalLink}
      dangerouslySetInnerHTML={{ __html: sanitized }}
    />
  );
});
