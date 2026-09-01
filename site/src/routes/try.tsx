import { createFileRoute } from "@tanstack/react-router";
import { useEffect, useRef, useState } from "react";
import {
  assemble,
  coerceArgs,
  compileExecute,
  fieldsOf,
  modelContext,
  parseDefinition,
  recap,
  toJsonValue,
  PRESETS,
  type Execute,
  type Field,
  type JsonObject,
  type ToolDefinition,
} from "../lib/webmcp";

export const Route = createFileRoute("/try")({ component: TryPage });

interface Registered {
  tool: ToolDefinition;
  run: Execute;
}

interface Run {
  id: number;
  at: Date;
  tool: string;
  args: JsonObject;
  result?: unknown;
  error?: string;
  ms?: number;
  via: "page" | "document.modelContext";
}

interface Note {
  text: string;
  fault: boolean;
}

const FLAG = "chrome://flags/#enable-webmcp-testing";

function clock(at: Date): string {
  return at.toLocaleTimeString([], { hour12: false });
}

function access(tool: ToolDefinition): { text: string; mutates: boolean } {
  return tool.annotations?.readOnlyHint === true
    ? { text: "Only reads", mutates: false }
    : { text: "Can change things", mutates: true };
}

function TryPage() {
  const [hasContext] = useState(() => modelContext() !== null);
  const [definition, setDefinition] = useState(PRESETS[0].definition);
  const [execute, setExecute] = useState(PRESETS[0].execute);
  const [definitionError, setDefinitionError] = useState<string | null>(null);
  const [executeError, setExecuteError] = useState<string | null>(null);
  const [registered, setRegistered] = useState<Registered | null>(null);
  const [note, setNote] = useState<Note | null>(null);
  const [raw, setRaw] = useState<Record<string, string>>({});
  const [rawJson, setRawJson] = useState("{}");
  const [pending, setPending] = useState(false);
  const [runs, setRuns] = useState<Run[]>([]);
  const registeredName = useRef<string | null>(null);

  // Leaving the page takes the tool with it, so the debugger never lists a ghost.
  useEffect(() => {
    return () => {
      const name = registeredName.current;
      if (name) {
        try {
          modelContext()?.unregisterTool?.(name);
        } catch {
          // Chrome without unregisterTool: the tool dies with the document.
        }
      }
    };
  }, []);

  function loadPreset(index: number) {
    setDefinition(PRESETS[index].definition);
    setExecute(PRESETS[index].execute);
    setDefinitionError(null);
    setExecuteError(null);
  }

  function register() {
    const parsed = parseDefinition(definition);
    const compiled = compileExecute(execute);
    setDefinitionError(parsed.ok ? null : parsed.error);
    setExecuteError(compiled.ok ? null : compiled.error);
    if (!parsed.ok || !compiled.ok) return;

    const tool = parsed.value;
    const run = compiled.value;
    const context = modelContext();
    const origin = window.location.origin;
    if (context) {
      try {
        if (registeredName.current) context.unregisterTool?.(registeredName.current);
        context.unregisterTool?.(tool.name);
        context.registerTool({
          ...tool,
          execute: (input: unknown) => run(coerceArgs(input)),
        });
        setNote({
          text: `Registered on this page. With the extension loaded, the debugger lists ${tool.name} under ${origin}.`,
          fault: false,
        });
      } catch (error) {
        setNote({
          text: `Chrome refused registerTool: ${(error as Error).message}. The tool still runs here, in the page.`,
          fault: true,
        });
      }
    } else {
      setNote({
        text: `WebMCP is off in this browser, so ${tool.name} lives in this page only. To see it in the debugger, enable ${FLAG} in Chrome 150 or newer and reload.`,
        fault: false,
      });
    }
    registeredName.current = tool.name;
    setRegistered({ tool, run });
    setRaw({});
    setRawJson("{}");
  }

  const fields: Field[] | null = registered ? fieldsOf(registered.tool.inputSchema) : [];
  const assembled = fields ? assemble(fields, raw) : null;
  let jsonError: string | null = null;
  let jsonArgs: JsonObject = {};
  if (!fields) {
    try {
      const parsed: unknown = JSON.parse(rawJson);
      if (typeof parsed === "object" && parsed !== null && !Array.isArray(parsed)) {
        jsonArgs = parsed as JsonObject;
      } else {
        jsonError = "arguments must be a JSON object";
      }
    } catch (error) {
      jsonError = `not valid JSON: ${(error as Error).message}`;
    }
  }
  const ready =
    registered !== null &&
    !pending &&
    (fields ? Object.keys(assembled!.errors).length === 0 : jsonError === null);

  async function runTool() {
    if (!registered || !ready) return;
    const args = fields ? assembled!.args : jsonArgs;
    const id = Date.now();
    const at = new Date();
    const started = performance.now();
    let via: Run["via"] = "page";
    setPending(true);
    setRuns((all) => [...all, { id, at, tool: registered.tool.name, args, via }]);
    try {
      let result: unknown;
      const context = modelContext();
      const listed = context?.executeTool
        ? (await context.getTools()).find((tool) => tool.name === registered.tool.name)
        : undefined;
      if (context?.executeTool && listed) {
        via = "document.modelContext";
        try {
          result = await context.executeTool(listed, args);
        } catch (error) {
          const message = (error as Error)?.message ?? "";
          // Some Chrome builds want the arguments as JSON text.
          if (!message.startsWith("Failed to parse input")) throw error;
          result = await context.executeTool(listed, JSON.stringify(args));
        }
        result = toJsonValue(result);
      } else {
        result = await registered.run(args);
      }
      const ms = Math.round(performance.now() - started);
      setRuns((all) => all.map((run) => (run.id === id ? { ...run, result: result ?? null, ms, via } : run)));
    } catch (error) {
      const ms = Math.round(performance.now() - started);
      const message = error instanceof Error ? error.message : String(error);
      setRuns((all) => all.map((run) => (run.id === id ? { ...run, error: message, ms, via } : run)));
    } finally {
      setPending(false);
    }
  }

  const last = registered ? [...runs].reverse().find((run) => run.tool === registered.tool.name) : undefined;

  return (
    <main id="main" className="mx-auto max-w-[640px] px-6 pb-16 pt-24 sm:px-0">
      <p className="t-label m-0">Try it</p>
      <h1 className="t-focus m-0 mt-2 font-normal">Register a tool on this page, then run it.</h1>
      <p className="mt-6 text-mute">
        This page is a WebMCP host of its own. Paste a tool, press Register, and
        it goes on <span className="text-ink">document.modelContext</span> the
        way any site would publish it. The debugger then lists it like any other
        page, and Run below goes through the same API.
      </p>
      <p className={`t-label m-0 mt-4 ${hasContext ? "" : "text-accent"}`}>
        {hasContext ? "WebMCP · available in this browser" : "WebMCP · off in this browser · tools run in the page only"}
      </p>

      <section className="mt-16" aria-labelledby="define">
        <div className="flex flex-wrap items-center justify-between gap-4 border-b border-hair pb-2">
          <h2 id="define" className="t-label m-0 font-normal">
            1 · Define
          </h2>
          <p className="m-0 flex items-center gap-4">
            <span className="t-label">Presets</span>
            {PRESETS.map((preset, index) => (
              <button
                key={preset.label}
                type="button"
                className="word"
                aria-pressed={definition === preset.definition && execute === preset.execute}
                onClick={() => loadPreset(index)}
              >
                {preset.label}
              </button>
            ))}
          </p>
        </div>

        <label className="mt-6 block">
          <span className="t-label block">
            Tool <span className="text-hair">·</span> JSON
          </span>
          <textarea
            className="field mt-2"
            rows={14}
            spellCheck={false}
            value={definition}
            onChange={(event) => setDefinition(event.target.value)}
            aria-invalid={definitionError !== null}
          />
        </label>
        {definitionError && <p className="m-0 mt-2 text-accent">{definitionError}</p>}

        <label className="mt-6 block">
          <span className="t-label block">
            execute(args) <span className="text-hair">·</span> body, may use await
          </span>
          <textarea
            className="field mt-2"
            rows={6}
            spellCheck={false}
            value={execute}
            onChange={(event) => setExecute(event.target.value)}
            aria-invalid={executeError !== null}
          />
        </label>
        {executeError && <p className="m-0 mt-2 text-accent">{executeError}</p>}

        <div className="mt-4 flex flex-wrap items-center gap-6">
          <button type="button" className="act" onClick={register}>
            Register ›
          </button>
          <span className="t-label">Runs as you, in this tab</span>
        </div>
        {note && <p className={`m-0 mt-4 ${note.fault ? "text-accent" : "text-mute"}`}>{note.text}</p>}
      </section>

      <section className="mt-16" aria-labelledby="run">
        <h2 id="run" className="t-label m-0 border-b border-hair pb-2 font-normal">
          2 · Run
        </h2>
        {!registered ? (
          <p className="mt-4 text-mute">Register a tool first. Run is dimmed until you do.</p>
        ) : (
          <div className="mt-6">
            <p className="t-focus m-0">{registered.tool.name}</p>
            {registered.tool.description && <p className="m-0 mt-1 text-mute">{registered.tool.description}</p>}
            <p className={`t-label m-0 mt-2 ${access(registered.tool).mutates ? "text-accent" : ""}`}>
              {access(registered.tool).text}
            </p>

            {fields ? (
              fields.length === 0 ? (
                <p className="mt-6 text-mute">This tool takes no arguments.</p>
              ) : (
                <div className="mt-6 flex flex-col gap-6">
                  {fields.map((field) => (
                    <FieldRow
                      key={field.name}
                      field={field}
                      value={raw[field.name] ?? ""}
                      error={assembled?.errors[field.name]}
                      onChange={(value) => setRaw((all) => ({ ...all, [field.name]: value }))}
                    />
                  ))}
                </div>
              )
            ) : (
              <label className="mt-6 block">
                <span className="t-label block">
                  Arguments <span className="text-hair">·</span> JSON, this schema has no simple form
                </span>
                <textarea
                  className="field mt-2"
                  rows={6}
                  spellCheck={false}
                  value={rawJson}
                  onChange={(event) => setRawJson(event.target.value)}
                  aria-invalid={jsonError !== null}
                />
                {jsonError && <span className="mt-2 block text-accent">{jsonError}</span>}
              </label>
            )}

            <div className="mt-4 flex flex-wrap items-center gap-6">
              <button type="button" className="act" disabled={!ready} onClick={runTool}>
                Run ›
              </button>
              {pending && <span className="t-label">Waiting for the page</span>}
            </div>

            {last && (
              <div className="mt-8 border-t border-hair pt-4" aria-live="polite">
                <p className="t-label m-0">
                  {last.error ? "Failed" : last.ms === undefined ? "Running" : "Result"}
                  {last.ms !== undefined && <span> · {last.ms}ms · via {last.via}</span>}
                </p>
                {recap(last.args) && <p className="m-0 mt-1 text-mute">{recap(last.args)}</p>}
                {last.error ? (
                  <p className="m-0 mt-3 text-accent">{last.error}</p>
                ) : last.ms !== undefined ? (
                  <pre className="mt-3">{JSON.stringify(last.result, null, 2)}</pre>
                ) : null}
              </div>
            )}
          </div>
        )}
      </section>

      <section className="mt-16" aria-labelledby="history">
        <div className="flex items-center justify-between border-b border-hair pb-2">
          <h2 id="history" className="t-label m-0 font-normal">
            3 · History
          </h2>
          <p className="t-label m-0">{runs.length} runs this session</p>
        </div>
        {runs.length === 0 ? (
          <p className="mt-4 text-mute">Nothing has run yet. Everything you run is recorded here.</p>
        ) : (
          <ol className="m-0 mt-4 list-none p-0">
            {[...runs].reverse().map((run) => (
              <li key={run.id} className="flex flex-wrap gap-x-4 gap-y-0 border-b border-hair py-2">
                <span className="text-mute">{clock(run.at)}</span>
                <span>{run.tool}</span>
                <span className="min-w-0 flex-1 truncate text-mute">{recap(run.args)}</span>
                <span className={run.error ? "text-accent" : "text-mute"}>
                  {run.error ?? (run.ms === undefined ? "running" : `${run.ms}ms`)}
                </span>
              </li>
            ))}
          </ol>
        )}
      </section>
    </main>
  );
}

interface FieldRowProps {
  field: Field;
  value: string;
  error?: string;
  onChange: (value: string) => void;
}

function kindLabel(field: Field): string {
  switch (field.kind) {
    case "text":
      return "string";
    case "choice":
      return `enum · ${field.options.length} values`;
    default:
      return field.kind;
  }
}

function FieldRow({ field, value, error, onChange }: FieldRowProps) {
  const id = `field-${field.name}`;
  const label = (
    <span className="t-label block">
      <span className="text-ink normal-case tracking-normal text-[14px]">{field.name}</span>
      <span className="text-hair"> · </span>
      {kindLabel(field)}
      {field.required && <span className="text-accent"> *</span>}
    </span>
  );
  const control = (() => {
    switch (field.kind) {
      case "boolean":
        return (
          <div className="flex gap-6" role="group" aria-labelledby={id}>
            {["false", "true"].map((option) => (
              <button
                key={option}
                type="button"
                className="word"
                aria-pressed={(value || "false") === option}
                onClick={() => onChange(option)}
              >
                {option}
              </button>
            ))}
          </div>
        );
      case "choice":
        return (
          <div className="flex flex-wrap gap-6" role="group" aria-labelledby={id}>
            {field.options.map((option) => (
              <button
                key={option}
                type="button"
                className="word"
                aria-pressed={value === option}
                onClick={() => onChange(value === option ? "" : option)}
              >
                {option}
              </button>
            ))}
          </div>
        );
      default:
        return (
          <input
            id={id}
            className="field mt-2"
            type="text"
            inputMode={field.kind === "text" ? "text" : "decimal"}
            value={value}
            placeholder={kindLabel(field)}
            onChange={(event) => onChange(event.target.value)}
            aria-invalid={error !== undefined}
          />
        );
    }
  })();
  const usesInput = field.kind === "text" || field.kind === "number" || field.kind === "integer";
  return (
    <div>
      {usesInput ? <label htmlFor={id}>{label}</label> : <span id={id}>{label}</span>}
      {field.description && <p className="m-0 mt-1 text-mute">{field.description}</p>}
      {control}
      {error && <p className="m-0 mt-2 text-accent">{error}</p>}
    </div>
  );
}
