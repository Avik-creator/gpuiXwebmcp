// The page's own WebMCP host: parse a pasted tool, register it, run it.
// Kept free of React so the rules can be read on their own.

export type JsonObject = Record<string, unknown>;

export interface ToolDefinition {
  name: string;
  title?: string;
  description: string;
  inputSchema: JsonObject;
  annotations?: { readOnlyHint?: boolean; untrustedContentHint?: boolean };
}

export type Execute = (args: JsonObject) => Promise<unknown>;

interface ModelContext {
  registerTool(tool: ToolDefinition & { execute: (args: unknown, options?: unknown) => unknown }): unknown;
  unregisterTool?(name: string): unknown;
  getTools(): Promise<Array<{ name: string }>>;
  executeTool?(tool: unknown, args: unknown, options?: unknown): Promise<unknown>;
}

/** Chrome's experimental API: on navigator per the spec, on document in early builds. */
export function modelContext(): ModelContext | null {
  if (typeof document === "undefined") return null;
  const nav = navigator as unknown as { modelContext?: ModelContext };
  const doc = document as unknown as { modelContext?: ModelContext };
  return nav.modelContext ?? doc.modelContext ?? null;
}

type Parsed<T> = { ok: true; value: T } | { ok: false; error: string };

const NAME = /^[A-Za-z0-9_.-]{1,64}$/;

function isObject(value: unknown): value is JsonObject {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

/** A tool definition as the page would pass it to registerTool, minus execute. */
export function parseDefinition(text: string): Parsed<ToolDefinition> {
  let raw: unknown;
  try {
    raw = JSON.parse(text);
  } catch (error) {
    return { ok: false, error: `not valid JSON: ${(error as Error).message}` };
  }
  if (!isObject(raw)) return { ok: false, error: "the definition must be a JSON object" };
  if (typeof raw.name !== "string" || !NAME.test(raw.name)) {
    return { ok: false, error: "name must be letters, digits, _ . or - (up to 64)" };
  }
  if (raw.description !== undefined && typeof raw.description !== "string") {
    return { ok: false, error: "description must be text" };
  }
  if (raw.inputSchema !== undefined && !isObject(raw.inputSchema)) {
    return { ok: false, error: "inputSchema must be a JSON Schema object" };
  }
  if (raw.annotations !== undefined && !isObject(raw.annotations)) {
    return { ok: false, error: "annotations must be an object" };
  }
  const annotations = (raw.annotations ?? {}) as JsonObject;
  return {
    ok: true,
    value: {
      name: raw.name,
      title: typeof raw.title === "string" ? raw.title : undefined,
      description: typeof raw.description === "string" ? raw.description : "",
      inputSchema: (raw.inputSchema as JsonObject | undefined) ?? { type: "object" },
      annotations: {
        readOnlyHint: typeof annotations.readOnlyHint === "boolean" ? annotations.readOnlyHint : undefined,
        untrustedContentHint:
          typeof annotations.untrustedContentHint === "boolean" ? annotations.untrustedContentHint : undefined,
      },
    },
  };
}

const AsyncFunction = Object.getPrototypeOf(async function () {}).constructor as new (
  ...parts: string[]
) => (args: JsonObject) => Promise<unknown>;

/** The body of `execute`, with `args` in scope; `await` works inside it. */
export function compileExecute(body: string): Parsed<Execute> {
  try {
    const run = new AsyncFunction("args", body);
    return { ok: true, value: run };
  } catch (error) {
    return { ok: false, error: `execute body: ${(error as Error).message}` };
  }
}

/** Some Chrome builds hand execute a JSON string rather than an object. */
export function coerceArgs(input: unknown): JsonObject {
  if (typeof input === "string") {
    try {
      const parsed = JSON.parse(input);
      return isObject(parsed) ? parsed : {};
    } catch {
      return {};
    }
  }
  return isObject(input) ? input : {};
}

/** Results come back as JSON text over the page API; read them as values. */
export function toJsonValue(result: unknown): unknown {
  if (typeof result !== "string") return result ?? null;
  try {
    return JSON.parse(result);
  } catch {
    return result;
  }
}

export type Field = {
  name: string;
  required: boolean;
  description?: string;
} & (
  | { kind: "text" | "number" | "integer" }
  | { kind: "boolean" }
  | { kind: "choice"; options: string[] }
);

/** Top-level fields the page can lay out, or null when the shape needs raw JSON. */
export function fieldsOf(schema: JsonObject): Field[] | null {
  const properties = schema.properties;
  if (properties === undefined) return [];
  if (!isObject(properties)) return null;
  const required = Array.isArray(schema.required) ? schema.required.filter((n) => typeof n === "string") : [];
  const fields: Field[] = [];
  for (const [name, property] of Object.entries(properties)) {
    if (!isObject(property)) return null;
    const base = {
      name,
      required: required.includes(name),
      description: typeof property.description === "string" ? property.description : undefined,
    };
    const options = Array.isArray(property.enum) ? property.enum : null;
    if (options) {
      if (!options.every((option) => typeof option === "string")) return null;
      fields.push({ ...base, kind: "choice", options: options as string[] });
      continue;
    }
    const type = Array.isArray(property.type)
      ? property.type.filter((t) => t !== "null")[0]
      : property.type;
    switch (type) {
      case "string":
        fields.push({ ...base, kind: "text" });
        break;
      case "number":
      case "integer":
        fields.push({ ...base, kind: type });
        break;
      case "boolean":
        fields.push({ ...base, kind: "boolean" });
        break;
      default:
        return null;
    }
  }
  return fields;
}

export interface Assembled {
  args: JsonObject;
  errors: Record<string, string>;
}

/** Widget text into arguments, with every complaint keyed by field. */
export function assemble(fields: Field[], raw: Record<string, string>): Assembled {
  const args: JsonObject = {};
  const errors: Record<string, string> = {};
  for (const field of fields) {
    const text = (raw[field.name] ?? "").trim();
    if (field.kind === "boolean") {
      args[field.name] = text === "true";
      continue;
    }
    if (text === "") {
      if (field.required) errors[field.name] = `${field.name} is required`;
      continue;
    }
    switch (field.kind) {
      case "text":
        args[field.name] = text;
        break;
      case "choice":
        if (field.options.includes(text)) args[field.name] = text;
        else errors[field.name] = `must be one of ${field.options.join(", ")}`;
        break;
      case "number": {
        const number = Number(text);
        if (Number.isFinite(number)) args[field.name] = number;
        else errors[field.name] = "must be a number";
        break;
      }
      case "integer": {
        const number = Number(text);
        if (Number.isInteger(number)) args[field.name] = number;
        else errors[field.name] = "must be a whole number";
        break;
      }
    }
  }
  return { args, errors };
}

/** One line saying what was sent, so a result never floats free of its input. */
export function recap(args: JsonObject): string {
  return Object.entries(args)
    .map(([key, value]) => {
      if (typeof value === "string") return `${key} ${value}`;
      if (Array.isArray(value)) return `${key} ${value.length} item(s)`;
      if (isObject(value)) return `${key} {…}`;
      return `${key} ${String(value)}`;
    })
    .join(" · ");
}

export interface Preset {
  label: string;
  definition: string;
  execute: string;
}

const pretty = (value: unknown) => JSON.stringify(value, null, 2);

/** The three tools the debugger's playground and demo site share. */
export const PRESETS: Preset[] = [
  {
    label: "create_note",
    definition: pretty({
      name: "create_note",
      title: "Create note",
      description: "Create a note from text",
      inputSchema: {
        type: "object",
        properties: { text: { type: "string", description: "What the note says" } },
        required: ["text"],
      },
      annotations: { readOnlyHint: false, untrustedContentHint: true },
    }),
    execute: [
      "const text = String(args.text ?? '').trim();",
      "if (!text) throw new Error('text is required');",
      "return { ok: true, text, savedAt: new Date().toISOString() };",
    ].join("\n"),
  },
  {
    label: "search_products",
    definition: pretty({
      name: "search_products",
      title: "Search products",
      description: "Search products by query",
      inputSchema: {
        type: "object",
        properties: {
          query: { type: "string" },
          limit: { type: "integer", description: "At most this many results" },
        },
        required: ["query"],
      },
      annotations: { readOnlyHint: true },
    }),
    execute: [
      "const books = [",
      "  { id: 'book-1', title: 'Programming GPUI', author: 'Zed Industries' },",
      "  { id: 'book-2', title: 'WebMCP in Practice', author: 'Chrome Team' },",
      "];",
      "const query = String(args.query ?? '').trim().toLowerCase();",
      "if (!query) throw new Error('query is required');",
      "const hits = books.filter((b) => b.title.toLowerCase().includes(query));",
      "return { query, results: hits.slice(0, args.limit ?? hits.length) };",
    ].join("\n"),
  },
  {
    label: "get_user",
    definition: pretty({
      name: "get_user",
      title: "Get user",
      description: "Return the current demo user profile",
      inputSchema: { type: "object", properties: {} },
      annotations: { readOnlyHint: true },
    }),
    execute: "return { id: 'user_1', name: 'Ada Lovelace', email: 'ada@localhost' };",
  },
];
