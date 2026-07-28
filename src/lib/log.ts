import { invoke } from "@tauri-apps/api/core";

import type { LogLevel } from "./types";

const RANK: Record<LogLevel, number> = {
  error: 0,
  warn: 1,
  info: 2,
  debug: 3,
  trace: 4,
};

const CONSOLE: Record<LogLevel, (...args: unknown[]) => void> = {
  error: console.error,
  warn: console.warn,
  info: console.info,
  debug: console.debug,
  trace: console.debug,
};

let threshold = RANK.info;

function serialize(data: unknown): string | null {
  if (data === undefined || data === null) return null;
  try {
    const text = JSON.stringify(data, (_key, value) =>
      value instanceof Error ? { name: value.name, message: value.message } : value,
    );
    return text === undefined ? String(data) : text;
  } catch {
    return String(data);
  }
}

function emit(level: LogLevel, scope: string, message: string, data?: unknown) {
  if (RANK[level] > threshold) return;

  if (data === undefined) {
    CONSOLE[level](`[${scope}] ${message}`);
  } else {
    CONSOLE[level](`[${scope}] ${message}`, data);
  }

  void invoke("frontend_log", {
    level,
    scope,
    message,
    data: serialize(data),
  }).catch(() => undefined);
}

export const log = {
  error: (scope: string, message: string, data?: unknown) =>
    emit("error", scope, message, data),
  warn: (scope: string, message: string, data?: unknown) =>
    emit("warn", scope, message, data),
  info: (scope: string, message: string, data?: unknown) =>
    emit("info", scope, message, data),
  debug: (scope: string, message: string, data?: unknown) =>
    emit("debug", scope, message, data),
  trace: (scope: string, message: string, data?: unknown) =>
    emit("trace", scope, message, data),
  setLevel: (level: LogLevel) => {
    threshold = RANK[level] ?? RANK.info;
  },
};

let handlersBound = false;

export function bindGlobalErrorHandlers() {
  if (handlersBound) return;
  handlersBound = true;

  window.addEventListener("error", (event) => {
    log.error("window", event.message || "uncaught error", {
      source: event.filename,
      line: event.lineno,
      column: event.colno,
      stack: event.error instanceof Error ? event.error.stack : undefined,
    });
  });

  window.addEventListener("unhandledrejection", (event) => {
    const reason = event.reason;
    log.error(
      "window",
      reason instanceof Error ? reason.message : `unhandled rejection: ${String(reason)}`,
      { stack: reason instanceof Error ? reason.stack : undefined },
    );
  });
}
