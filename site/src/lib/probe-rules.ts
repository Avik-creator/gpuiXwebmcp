// The site check, minus the network: URL rules and what counts as a WebMCP sign.
// Shared by the dev-server middleware and the page, so they cannot disagree.

export const MARKERS = ["modelContext", "registerTool", "provideContext", "toolchange"] as const;

export type ProbeResult =
  | {
      ok: true;
      url: string;
      finalUrl: string;
      status: number;
      title: string;
      markers: string[];
      names: string[];
      scripts: number;
      bytes: number;
    }
  | { ok: false; error: string };

const BLOCKED = [
  "javascript:",
  "data:",
  "file:",
  "vbscript:",
  "blob:",
  "chrome:",
  "chrome-extension:",
  "about:",
  "view-source:",
  "ws:",
  "wss:",
  "ftp:",
];

type Normalized = { ok: true; url: string } | { ok: false; error: string };

/** The debugger's site-field rules: http(s) only, https unless the host looks local. */
export function normalizeSiteUrl(raw: string): Normalized {
  const trimmed = raw.trim();
  if (!trimmed) return { ok: false, error: "enter a site url" };
  if (/[\s\u0000-\u001f]/.test(trimmed)) return { ok: false, error: "url must not contain spaces" };
  const lower = trimmed.toLowerCase();
  if (BLOCKED.some((scheme) => lower.startsWith(scheme))) return { ok: false, error: "only http and https urls" };
  let candidate = trimmed;
  if (!trimmed.includes("://")) {
    const hostport = trimmed.split(/[/?#]/)[0];
    const host = (hostport.includes("]") ? hostport.slice(0, hostport.indexOf("]") + 1) : hostport.replace(/:\d+$/, "")).toLowerCase();
    const local = host === "localhost" || host === "127.0.0.1" || host === "[::1]";
    candidate = `${local ? "http" : "https"}://${trimmed}`;
  }
  let parsed: URL;
  try {
    parsed = new URL(candidate);
  } catch {
    return { ok: false, error: "invalid url" };
  }
  if (parsed.protocol !== "http:" && parsed.protocol !== "https:") return { ok: false, error: "only http and https urls" };
  if (!parsed.hostname) return { ok: false, error: "url needs a host" };
  return { ok: true, url: parsed.href };
}

/** Same-origin scripts the page loads, in order, without repeats. */
export function scriptSources(html: string, base: string): string[] {
  const origin = new URL(base).origin;
  const out: string[] = [];
  for (const match of html.matchAll(/<script\b[^>]*\bsrc\s*=\s*["']([^"']+)["']/gi)) {
    let resolved: URL;
    try {
      resolved = new URL(match[1], base);
    } catch {
      continue;
    }
    if (resolved.origin !== origin) continue;
    if (!out.includes(resolved.href)) out.push(resolved.href);
  }
  return out;
}

/** The bodies of inline scripts. */
export function inlineScripts(html: string): string[] {
  const out: string[] = [];
  for (const match of html.matchAll(/<script\b([^>]*)>([\s\S]*?)<\/script>/gi)) {
    if (/\bsrc\s*=/i.test(match[1])) continue;
    if (match[2].trim()) out.push(match[2]);
  }
  return out;
}

export function findMarkers(text: string): string[] {
  return MARKERS.filter((marker) => text.includes(marker));
}

const NAME = /["'`]?name["'`]?\s*:\s*["'`]([A-Za-z0-9_.-]{1,64})["'`]/g;

/** Names that sit just before an `inputSchema`: the shape of a tool object literal. */
export function toolNames(text: string): string[] {
  const out: string[] = [];
  let at = text.indexOf("inputSchema");
  while (at !== -1) {
    const window = text.slice(Math.max(0, at - 400), at);
    let last: string | null = null;
    for (const match of window.matchAll(NAME)) last = match[1];
    if (last && !out.includes(last)) out.push(last);
    at = text.indexOf("inputSchema", at + 1);
  }
  return out;
}

export function titleOf(html: string): string {
  const match = /<title[^>]*>([\s\S]*?)<\/title>/i.exec(html);
  return match ? match[1].replace(/\s+/g, " ").trim().slice(0, 120) : "";
}
