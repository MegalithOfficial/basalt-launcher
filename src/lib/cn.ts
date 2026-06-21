/** Tiny classNames joiner — keeps JSX tidy without pulling in a dependency. */
export function cn(...parts: Array<string | false | null | undefined>): string {
  return parts.filter(Boolean).join(" ");
}
