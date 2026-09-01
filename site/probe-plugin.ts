// GET /api/probe?url=… on the dev and preview servers: read a site's shipped
// code and say whether it references WebMCP. It cannot run the page.
import type { IncomingMessage, ServerResponse } from "node:http";
import type { Plugin } from "vite";
import {
  findMarkers,
  inlineScripts,
  normalizeSiteUrl,
  scriptSources,
  titleOf,
  toolNames,
  type ProbeResult,
} from "./src/lib/probe-rules";

const MAX_BYTES = 2_000_000;
const MAX_SCRIPTS = 12;
const TIMEOUT_MS = 8_000;

async function readCapped(url: string, signal: AbortSignal): Promise<{ status: number; finalUrl: string; text: string }> {
  const response = await fetch(url, {
    signal,
    redirect: "follow",
    headers: { accept: "text/html,text/javascript,*/*", "user-agent": "gpuiXwebmcp-probe" },
  });
  const chunks: Uint8Array[] = [];
  let bytes = 0;
  const reader = response.body?.getReader();
  if (reader) {
    while (bytes < MAX_BYTES) {
      const { done, value } = await reader.read();
      if (done || !value) break;
      chunks.push(value);
      bytes += value.byteLength;
    }
    await reader.cancel().catch(() => {});
  }
  const text = new TextDecoder("utf-8", { fatal: false }).decode(Buffer.concat(chunks));
  return { status: response.status, finalUrl: response.url || url, text };
}

export async function probe(url: string): Promise<ProbeResult> {
  const controller = new AbortController();
  const timer = setTimeout(() => controller.abort(), TIMEOUT_MS);
  try {
    const page = await readCapped(url, controller.signal);
    let corpus = inlineScripts(page.text).join("\n");
    let bytes = page.text.length;
    const sources = scriptSources(page.text, page.finalUrl).slice(0, MAX_SCRIPTS);
    let scripts = 0;
    for (const source of sources) {
      try {
        const script = await readCapped(source, controller.signal);
        corpus += `\n${script.text}`;
        bytes += script.text.length;
        scripts += 1;
      } catch {
        // One missing script must not sink the whole check.
      }
    }
    return {
      ok: true,
      url,
      finalUrl: page.finalUrl,
      status: page.status,
      title: titleOf(page.text),
      markers: findMarkers(corpus),
      names: toolNames(corpus),
      scripts,
      bytes,
    };
  } catch (error) {
    const message = controller.signal.aborted ? `no answer within ${TIMEOUT_MS / 1000}s` : (error as Error).message;
    return { ok: false, error: `could not read ${url}: ${message}` };
  } finally {
    clearTimeout(timer);
  }
}

export function probePlugin(): Plugin {
  const handle = async (req: IncomingMessage, res: ServerResponse, next: () => void) => {
    if (!req.url?.startsWith("/api/probe")) return next();
    res.setHeader("content-type", "application/json");
    const raw = new URL(req.url, "http://localhost").searchParams.get("url") ?? "";
    const normalized = normalizeSiteUrl(raw);
    if (!normalized.ok) {
      res.statusCode = 400;
      res.end(JSON.stringify({ ok: false, error: normalized.error } satisfies ProbeResult));
      return;
    }
    const result = await probe(normalized.url);
    res.statusCode = result.ok ? 200 : 502;
    res.end(JSON.stringify(result));
  };
  return {
    name: "webmcp-probe",
    configureServer(server) {
      server.middlewares.use(handle);
    },
    configurePreviewServer(server) {
      server.middlewares.use(handle);
    },
  };
}
