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

export function Markdown({
  body,
  format,
  className,
}: {
  body: string;
  format: "markdown" | "html";
  className?: string;
}) {
  if (format === "markdown") {
    return (
      <div className={cn("prose-basalt", className)}>
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
      dangerouslySetInnerHTML={{ __html: DOMPurify.sanitize(body) }}
    />
  );
}
