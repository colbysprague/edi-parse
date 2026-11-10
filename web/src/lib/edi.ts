import init, { parse_files } from "@/pkg/edi_parse_wasm.js"

/** Mock filesystem: input path -> file contents. */
export type FileSystem = Record<string, string>

export type Output =
  | { kind: "json"; text: string }
  | { kind: "error"; input: string; error: string }

export type RunResult = {
  outputs: Map<string, Output>
  ms: number
  files: number
  failed: number
  validationErrors: number
}

type WasmResult = {
  files: { path: string; output?: string; error?: string }[]
  summary: string
  validation_errors: string
}

export async function load(): Promise<FileSystem> {
  const [fs] = await Promise.all([
    fetch(`${import.meta.env.BASE_URL}fs.json`).then((r) => r.json() as Promise<FileSystem>),
    init(),
  ])
  return fs
}

export const outputPath = (input: string) => input.replace(/\.[^./]+$/, "") + ".json"

export const SUMMARY = "summary.json"
export const VALIDATION_ERRORS = "validation_errors.json"

/** Parse the selected files the way `edi-parse parse <dir> --out` would. */
export function run(fs: FileSystem, selected: string[]): RunResult {
  // Empty files (e.g. a new file not typed into yet) are skipped, not failed.
  const inputs = [...selected]
    .sort()
    .filter((path) => fs[path]?.trim())
    .map((path) => ({ path, contents: fs[path] }))
  if (!inputs.length) return { outputs: new Map(), ms: 0, files: 0, failed: 0, validationErrors: 0 }
  const t0 = performance.now()
  const result = JSON.parse(parse_files(JSON.stringify(inputs))) as WasmResult
  const ms = performance.now() - t0

  const outputs = new Map<string, Output>()
  let failed = 0
  for (const f of result.files) {
    if (f.error !== undefined) {
      failed++
      outputs.set(outputPath(f.path), { kind: "error", input: f.path, error: f.error })
    } else {
      outputs.set(outputPath(f.path), { kind: "json", text: f.output! })
    }
  }
  if (inputs.length) {
    outputs.set(SUMMARY, { kind: "json", text: result.summary })
    outputs.set(VALIDATION_ERRORS, { kind: "json", text: result.validation_errors })
  }

  return {
    outputs,
    ms,
    files: inputs.length,
    failed,
    validationErrors: inputs.length ? (JSON.parse(result.validation_errors) as unknown[]).length : 0,
  }
}

// ---------- trees ----------

export type TreeNode = {
  name: string
  path: string
  children: TreeNode[] | null // null for files
}

export function buildTree(paths: string[], pinLast: string[] = []): TreeNode[] {
  type Mutable = { name: string; path: string; children: Map<string, Mutable> | null }
  const root: Mutable = { name: "", path: "", children: new Map() }
  for (const path of paths) {
    let node = root
    const parts = path.split("/")
    parts.forEach((part, i) => {
      if (!node.children!.has(part)) {
        node.children!.set(part, {
          name: part,
          path: parts.slice(0, i + 1).join("/"),
          children: i < parts.length - 1 ? new Map() : null,
        })
      }
      node = node.children!.get(part)!
    })
  }

  const freeze = (n: Mutable, depth: number): TreeNode[] =>
    [...n.children!.values()]
      .sort((a, b) => {
        if (depth === 0) {
          const pa = pinLast.indexOf(a.path)
          const pb = pinLast.indexOf(b.path)
          if (pa !== pb) return pa - pb
        }
        if (!!a.children !== !!b.children) return a.children ? -1 : 1
        return a.name.localeCompare(b.name, undefined, { numeric: true })
      })
      .map((c) => ({ name: c.name, path: c.path, children: c.children ? freeze(c, depth + 1) : null }))

  return freeze(root, 0)
}

export function leaves(node: TreeNode): string[] {
  return node.children ? node.children.flatMap(leaves) : [node.path]
}

// ---------- highlighting (returns escaped HTML per line) ----------

const escape = (s: string) =>
  s.replace(/[&<>]/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;" })[c]!)

const JSON_TOKEN = /("(?:\\.|[^\\"])*")(\s*:)?|\b(true|false|null)\b|(-?\d+(?:\.\d+)?(?:[eE][+-]?\d+)?)|([{}[\],])/g

export function highlightJson(text: string): string[] {
  return text.split("\n").map((line) =>
    escape(line).replace(JSON_TOKEN, (_m, str, colon, lit, num, punct) => {
      if (str) return colon ? `<span class="tok-key">${str}</span>${colon}` : `<span class="tok-str">${str}</span>`
      if (lit) return `<span class="tok-lit">${lit}</span>`
      if (num) return `<span class="tok-num">${num}</span>`
      return `<span class="tok-punct">${punct}</span>`
    }),
  )
}

/**
 * Highlight EDI text without changing it: the output has exactly the same
 * characters as the input, so it can sit under a textarea.
 */
export function highlightEdiRaw(text: string): string {
  const { element, segment } = ediDelimiters(text)
  let out = ""
  let run = ""
  let atStart = true
  const flush = () => {
    out += escape(run)
    run = ""
  }

  for (let i = 0; i < text.length; ) {
    if (atStart) {
      while (i < text.length && /\s/.test(text[i])) run += text[i++]
      flush()
      let id = ""
      while (i < text.length && text[i] !== element && text[i] !== segment && text[i] !== "\n") id += text[i++]
      if (id) out += `<span class="tok-seg">${escape(id)}</span>`
      atStart = false
      continue
    }
    const c = text[i++]
    if (c === element || c === segment) {
      flush()
      out += `<span class="tok-sep">${escape(c)}</span>`
      if (c === segment) atStart = true
    } else {
      run += c
    }
  }
  flush()
  return out
}

/** Put each segment on its own line. The parser ignores line breaks between segments. */
export function formatEdi(text: string): string {
  const { segment } = ediDelimiters(text)
  return (
    text
      .split(segment)
      .map((s) => s.replace(/^[\r\n]+|[\r\n]+$/g, ""))
      .filter((s) => s.length)
      .join(`${segment}\n`) + (text.trimEnd().endsWith(segment) ? `${segment}\n` : "")
  )
}

function ediDelimiters(text: string) {
  const t = text.replace(/^\s+/, "")
  if (t.startsWith("ISA") && t.length >= 106) return { element: t[3], segment: t[105] }
  return { element: "*", segment: "~" }
}

/** One line per segment, with the segment ID and delimiters marked. */
export function highlightEdi(text: string): string[] {
  const { element, segment } = ediDelimiters(text)
  const endsWithTerminator = text.trimEnd().endsWith(segment)
  const segments = text
    .split(segment)
    .map((s) => s.replace(/^[\r\n]+|[\r\n]+$/g, ""))
    .filter((s) => s.length)
  const sep = `<span class="tok-sep">${escape(element)}</span>`
  const term = `<span class="tok-sep">${escape(segment)}</span>`
  return segments.map((s, i) => {
    const [id, ...rest] = s.split(element)
    const tail = rest.length ? sep + rest.map(escape).join(sep) : ""
    const end = i < segments.length - 1 || endsWithTerminator ? term : ""
    return `<span class="tok-seg">${escape(id)}</span>${tail}${end}`
  })
}
