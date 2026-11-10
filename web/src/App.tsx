import {
  Check,
  ChevronRight,
  Copy,
  FilePlus2,
  FileSearch,
  FileUp,
  Loader2,
  RotateCcw,
  Trash2,
  Upload,
  WrapText,
  X,
} from "lucide-react"
import { useCallback, useEffect, useMemo, useRef, useState } from "react"

import { CodeView } from "@/components/code-view"
import { EdiEditor } from "@/components/edi-editor"
import { FileTree } from "@/components/file-tree"
import { ThemeToggle } from "@/components/theme-toggle"
import { Button } from "@/components/ui/button"
import { ResizableHandle, ResizablePanel, ResizablePanelGroup } from "@/components/ui/resizable"
import { ScrollArea } from "@/components/ui/scroll-area"
import { Separator } from "@/components/ui/separator"
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip"
import {
  buildTree,
  formatEdi,
  load,
  outputPath,
  run,
  SUMMARY,
  VALIDATION_ERRORS,
  type FileSystem,
  type Output,
  type RunResult,
} from "@/lib/edi"

const REPO = "https://github.com/colbysprague/edi-parse"
const DEFAULT_SELECTION = ["samples/837p_multi_claim.edi", "samples/835_multi_clp.edi"]
const UPLOADS = "uploads"

function selectionFromHash(fs: FileSystem): string[] {
  const raw = decodeURIComponent(location.hash.slice(1))
  return raw ? raw.split(",").filter((p) => p in fs) : []
}

function parentDirs(path: string): string[] {
  const parts = path.split("/")
  return parts.slice(1).map((_, i) => parts.slice(0, i + 1).join("/"))
}

/** `uploads/<name>`, with a numeric suffix if that path is taken. */
function uniqueUploadPath(fs: FileSystem, name: string): string {
  const clean = name.replace(/[/\\]/g, "_") || "untitled.edi"
  const dot = clean.lastIndexOf(".")
  const [base, ext] = dot > 0 ? [clean.slice(0, dot), clean.slice(dot)] : [clean, ".edi"]
  let path = `${UPLOADS}/${base}${ext}`
  for (let i = 2; path in fs; i++) path = `${UPLOADS}/${base}-${i}${ext}`
  return path
}

function useIsMobile() {
  const query = "(max-width: 767px)"
  const [mobile, setMobile] = useState(() => matchMedia(query).matches)
  useEffect(() => {
    const mql = matchMedia(query)
    const onChange = () => setMobile(mql.matches)
    mql.addEventListener("change", onChange)
    return () => mql.removeEventListener("change", onChange)
  }, [])
  return mobile
}

const isTextField = (el: EventTarget | null) =>
  el instanceof HTMLElement && (el.isContentEditable || /^(INPUT|TEXTAREA|SELECT)$/.test(el.tagName))

export default function App() {
  const [original, setOriginal] = useState<FileSystem | null>(null)
  const [fs, setFs] = useState<FileSystem>({})
  const [loadError, setLoadError] = useState<string | null>(null)
  const [selected, setSelected] = useState<Set<string>>(new Set())
  const [expanded, setExpanded] = useState<Set<string>>(new Set(["x12", "samples", "invalid", UPLOADS]))
  const [outCollapsed, setOutCollapsed] = useState<Set<string>>(new Set())
  /** The file in the input pane. */
  const [activeInput, setActiveInput] = useState<string | null>(null)
  /** `summary.json` or `validation_errors.json` in the output pane; null shows the active input's output. */
  const [outputOverride, setOutputOverride] = useState<string | null>(null)
  const [dragging, setDragging] = useState(false)
  // Shown again on every page load.
  const [showNotice, setShowNotice] = useState(true)
  const fileInput = useRef<HTMLInputElement>(null)
  const mobile = useIsMobile()

  useEffect(() => {
    load()
      .then((fs) => {
        const initial = selectionFromHash(fs)
        const sel = initial.length ? initial : DEFAULT_SELECTION
        setOriginal(fs)
        setFs(fs)
        setSelected(new Set(sel))
        setExpanded((prev) => new Set([...prev, ...sel.flatMap(parentDirs)]))
        setActiveInput([...sel].sort()[0] ?? null)
      })
      .catch((e: Error) => setLoadError(e.message))
  }, [])

  // `run` needs the wasm module, which is ready once `original` is set.
  const result = useMemo(() => run(original ? fs : {}, original ? [...selected] : []), [original, fs, selected])

  // Parse the active input on its own, whether or not it's selected.
  const activeOutput: Output | undefined = useMemo(() => {
    if (!original || !activeInput || !(activeInput in fs) || !fs[activeInput].trim()) return undefined
    return run(fs, [activeInput]).outputs.get(outputPath(activeInput))
  }, [original, fs, activeInput])

  // Keep the URL in sync with the selected bundled files, so a view can be shared.
  useEffect(() => {
    if (!original) return
    const hash = [...selected].filter((p) => p in original).sort().join(",")
    history.replaceState(null, "", hash ? `#${encodeURIComponent(hash)}` : location.pathname)
  }, [original, selected])

  const openInput = useCallback((path: string) => {
    setActiveInput(path)
    setOutputOverride(null)
    setExpanded((prev) => new Set([...prev, ...parentDirs(path)]))
  }, [])

  const openOutput = (path: string) => {
    if (path === SUMMARY || path === VALIDATION_ERRORS) {
      setOutputOverride(path)
      return
    }
    const input = [...selected].find((p) => outputPath(p) === path)
    if (input) openInput(input)
  }

  const addFiles = useCallback(
    (files: { name: string; text: string }[]) => {
      if (!files.length) return
      const next = { ...fs }
      const paths = files.map((f) => {
        const path = uniqueUploadPath(next, f.name)
        next[path] = f.text
        return path
      })
      setFs(next)
      setSelected((prev) => new Set([...prev, ...paths]))
      openInput(paths[0])
    },
    [fs, openInput],
  )

  const readFiles = useCallback(
    async (list: FileList | File[]) => {
      const files = await Promise.all([...list].map(async (f) => ({ name: f.name, text: await f.text() })))
      addFiles(files)
    },
    [addFiles],
  )

  // Paste anywhere outside a text field to add the clipboard as a new file.
  useEffect(() => {
    const onPaste = (e: ClipboardEvent) => {
      if (isTextField(e.target)) return
      if (e.clipboardData?.files.length) {
        e.preventDefault()
        void readFiles(e.clipboardData.files)
        return
      }
      const text = e.clipboardData?.getData("text")
      if (text?.trim()) {
        e.preventDefault()
        addFiles([{ name: "pasted.edi", text }])
      }
    }
    document.addEventListener("paste", onPaste)
    return () => document.removeEventListener("paste", onPaste)
  }, [addFiles, readFiles])

  // Drop files anywhere on the page.
  useEffect(() => {
    let depth = 0
    const hasFiles = (e: DragEvent) => e.dataTransfer?.types.includes("Files")
    const onEnter = (e: DragEvent) => {
      if (!hasFiles(e)) return
      depth++
      setDragging(true)
    }
    const onLeave = (e: DragEvent) => {
      if (!hasFiles(e)) return
      depth = Math.max(0, depth - 1)
      if (!depth) setDragging(false)
    }
    const onOver = (e: DragEvent) => {
      if (hasFiles(e)) e.preventDefault()
    }
    const onDrop = (e: DragEvent) => {
      if (!hasFiles(e)) return
      e.preventDefault()
      depth = 0
      setDragging(false)
      if (e.dataTransfer?.files.length) void readFiles(e.dataTransfer.files)
    }
    window.addEventListener("dragenter", onEnter)
    window.addEventListener("dragleave", onLeave)
    window.addEventListener("dragover", onOver)
    window.addEventListener("drop", onDrop)
    return () => {
      window.removeEventListener("dragenter", onEnter)
      window.removeEventListener("dragleave", onLeave)
      window.removeEventListener("dragover", onOver)
      window.removeEventListener("drop", onDrop)
    }
  }, [readFiles])

  const inputTree = useMemo(() => buildTree(Object.keys(fs)), [fs])
  const outputTree = useMemo(
    () => buildTree([...result.outputs.keys()], [SUMMARY, VALIDATION_ERRORS]),
    [result],
  )
  const failedOutputs = useMemo(
    () => new Set([...result.outputs].filter(([, o]) => o.kind === "error").map(([p]) => p)),
    [result],
  )
  const modifiedInputs = useMemo(
    () => new Set(Object.keys(fs).filter((p) => original && p in original && fs[p] !== original[p])),
    [fs, original],
  )

  const onSelect = useCallback(
    (paths: string[], on: boolean) => {
      setSelected((prev) => {
        const next = new Set(prev)
        for (const p of paths) {
          if (on) next.add(p)
          else next.delete(p)
        }
        return next
      })
      if (on && paths.length === 1) openInput(paths[0])
    },
    [openInput],
  )

  const toggle = (setter: typeof setExpanded) => (path: string) =>
    setter((prev) => {
      const next = new Set(prev)
      if (next.has(path)) next.delete(path)
      else next.add(path)
      return next
    })

  const setContents = (path: string, text: string) => setFs((prev) => ({ ...prev, [path]: text }))

  const deleteUpload = (path: string) => {
    setFs(({ [path]: _, ...rest }) => rest)
    setSelected((prev) => {
      const next = new Set(prev)
      next.delete(path)
      return next
    })
    if (activeInput === path) setActiveInput(null)
  }

  if (loadError) {
    return <Centered>Failed to load: {loadError}</Centered>
  }
  if (!original) {
    return (
      <Centered>
        <Loader2 className="size-4 animate-spin" /> Loading
      </Centered>
    )
  }

  const outputShown = outputOverride && result.outputs.has(outputOverride) ? outputOverride : null
  const activeOutPath = outputShown ?? (activeInput ? outputPath(activeInput) : undefined)

  const sidebar = (
    <ResizablePanelGroup orientation="vertical" className="bg-sidebar">
      <ResizablePanel defaultSize="62" minSize="20">
        <Pane
          title="Files"
          meta={`${selected.size} selected`}
          action={
            <div className="flex items-center gap-0.5">
              <IconButton label="New file" onClick={() => addFiles([{ name: "untitled.edi", text: "" }])}>
                <FilePlus2 />
              </IconButton>
              <IconButton label="Open files" onClick={() => fileInput.current?.click()}>
                <FileUp />
              </IconButton>
              <Button
                variant="ghost"
                size="xs"
                disabled={!selected.size}
                onClick={() => setSelected(new Set())}
              >
                Clear
              </Button>
            </div>
          }
        >
          <FileTree
            nodes={inputTree}
            expanded={expanded}
            onToggleExpanded={toggle(setExpanded)}
            activePath={activeInput ?? undefined}
            onOpen={openInput}
            selected={selected}
            onSelect={onSelect}
            modified={modifiedInputs}
          />
        </Pane>
      </ResizablePanel>
      <ResizableHandle />
      <ResizablePanel defaultSize="38" minSize="15">
        <Pane title="Output" meta="out/">
          {outputTree.length ? (
            <FileTree
              nodes={outputTree}
              expanded={outCollapsed}
              invertExpanded
              onToggleExpanded={toggle(setOutCollapsed)}
              activePath={activeOutPath}
              onOpen={openOutput}
              failed={failedOutputs}
            />
          ) : (
            <p className="px-2 py-1 text-sm text-muted-foreground">Tick files to parse them together</p>
          )}
        </Pane>
      </ResizablePanel>
    </ResizablePanelGroup>
  )

  const isUpload = !!activeInput && !(activeInput in original)
  const inputPane = (
    <InputPane
      path={activeInput}
      text={activeInput ? (fs[activeInput] ?? "") : ""}
      modified={!!activeInput && modifiedInputs.has(activeInput)}
      uploaded={isUpload}
      onChange={(t) => activeInput && setContents(activeInput, t)}
      onFormat={() => activeInput && setContents(activeInput, formatEdi(fs[activeInput]))}
      onReset={() => activeInput && setContents(activeInput, original[activeInput])}
      onDelete={() => activeInput && deleteUpload(activeInput)}
      onNew={() => addFiles([{ name: "untitled.edi", text: "" }])}
      onOpen={() => fileInput.current?.click()}
    />
  )

  const outputPane = outputShown ? (
    <OutputPane path={outputShown} output={result.outputs.get(outputShown)} />
  ) : (
    <OutputPane path={activeInput ? outputPath(activeInput) : null} output={activeOutput} />
  )

  return (
    <div className="flex h-dvh flex-col">
      {showNotice && (
        <div
          role="note"
          className="flex shrink-0 items-center gap-2 border-b border-amber-500/30 bg-amber-500/10 py-1 pr-1 pl-4 text-xs text-amber-900 dark:text-amber-200"
        >
          <span className="flex-1 text-center">
            For demonstration purposes only. Please do not enter protected health information (PHI).
          </span>
          <Button
            variant="ghost"
            size="icon-xs"
            aria-label="Dismiss"
            onClick={() => setShowNotice(false)}
            className="text-amber-900 hover:bg-amber-500/15 hover:text-amber-950 dark:text-amber-200 dark:hover:text-amber-100"
          >
            <X />
          </Button>
        </div>
      )}
      <header className="flex h-12 shrink-0 items-center gap-3 border-b px-4">
        <span className="font-mono text-sm font-semibold tracking-tight">edi-parse</span>
        <Separator orientation="vertical" className="my-auto h-4" />
        <Stats result={result} />
        <div className="ml-auto flex items-center gap-1">
          <ThemeToggle />
          <Button
            variant="outline"
            size="sm"
            render={<a href={REPO} target="_blank" rel="noopener" />}
            nativeButton={false}
          >
            <GitHubIcon />
            <span className="max-sm:hidden">GitHub</span>
          </Button>
        </div>
      </header>

      <ResizablePanelGroup orientation={mobile ? "vertical" : "horizontal"} className="min-h-0 flex-1">
        <ResizablePanel defaultSize={mobile ? "30" : "22"} minSize={mobile ? "15" : "14"}>
          {sidebar}
        </ResizablePanel>
        <ResizableHandle />
        <ResizablePanel defaultSize={mobile ? "70" : "78"} minSize="30">
          <ResizablePanelGroup orientation={mobile ? "vertical" : "horizontal"}>
            <ResizablePanel defaultSize="50" minSize="20">
              {inputPane}
            </ResizablePanel>
            <ResizableHandle />
            <ResizablePanel defaultSize="50" minSize="20">
              {outputPane}
            </ResizablePanel>
          </ResizablePanelGroup>
        </ResizablePanel>
      </ResizablePanelGroup>

      <input
        ref={fileInput}
        type="file"
        multiple
        accept=".edi,.x12,.txt,text/plain"
        className="hidden"
        onChange={(e) => {
          if (e.target.files) void readFiles(e.target.files)
          e.target.value = ""
        }}
      />

      {dragging && (
        <div className="pointer-events-none fixed inset-0 z-50 flex items-center justify-center bg-background/80 backdrop-blur-sm">
          <div className="flex flex-col items-center gap-2 rounded-xl border-2 border-dashed border-primary/40 px-12 py-10 text-sm">
            <Upload className="size-6 text-muted-foreground" />
            Drop EDI files to parse them
          </div>
        </div>
      )}
    </div>
  )
}

// ---------- panes ----------

function InputPane(props: {
  path: string | null
  text: string
  modified: boolean
  uploaded: boolean
  onChange: (text: string) => void
  onFormat: () => void
  onReset: () => void
  onDelete: () => void
  onNew: () => void
  onOpen: () => void
}) {
  if (!props.path) {
    return (
      <div className="flex h-full flex-col">
        <PaneHeader segments={["input"]} />
        <div className="flex flex-1 flex-col items-center justify-center gap-3 p-6 text-center text-sm text-muted-foreground">
          <Upload className="size-8 text-muted-foreground/50" />
          <p>Pick a file on the left, drop or paste EDI anywhere, or</p>
          <div className="flex gap-2">
            <Button variant="outline" size="sm" onClick={props.onNew}>
              <FilePlus2 /> New file
            </Button>
            <Button variant="outline" size="sm" onClick={props.onOpen}>
              <FileUp /> Open files
            </Button>
          </div>
        </div>
      </div>
    )
  }

  return (
    <div className="flex h-full flex-col">
      <PaneHeader
        segments={props.path.split("/")}
        actions={
          <>
            <IconButton label="One segment per line" onClick={props.onFormat} disabled={!props.text}>
              <WrapText />
            </IconButton>
            {props.modified && (
              <IconButton label="Reset to original" onClick={props.onReset}>
                <RotateCcw />
              </IconButton>
            )}
            {props.uploaded && (
              <IconButton label="Delete file" onClick={props.onDelete}>
                <Trash2 />
              </IconButton>
            )}
            <CopyButton text={props.text} />
          </>
        }
      />
      <div className="min-h-0 flex-1">
        <EdiEditor
          key={props.path}
          value={props.text}
          onChange={props.onChange}
          placeholder="Paste or type EDI here"
        />
      </div>
    </div>
  )
}

function OutputPane({ path, output }: { path: string | null; output: Output | undefined }) {
  const text = output?.kind === "json" ? output.text : output ? `${output.input}: ${output.error}` : ""
  return (
    <div className="flex h-full flex-col">
      <PaneHeader
        segments={path ? ["out", ...path.split("/")] : ["output"]}
        actions={output && <CopyButton text={text} />}
      />
      <div className="min-h-0 flex-1">
        {!output ? (
          <div className="flex h-full flex-col items-center justify-center gap-2 text-sm text-muted-foreground">
            <FileSearch className="size-8 text-muted-foreground/50" />
            Parsed JSON shows here
          </div>
        ) : output.kind === "error" ? (
          <div className="p-4">
            <div className="rounded-lg border border-destructive/30 bg-destructive/5 p-4 text-sm">
              <p className="font-medium text-destructive">Parse failed</p>
              <p className="mt-1 font-mono text-[13px] break-words text-foreground/80">{output.error}</p>
              <p className="mt-2 text-xs text-muted-foreground">No output file is written for this input.</p>
            </div>
          </div>
        ) : (
          <CodeView language="json" text={output.text} />
        )}
      </div>
    </div>
  )
}

// ---------- small pieces ----------

function PaneHeader(props: { segments: string[]; actions?: React.ReactNode }) {
  return (
    <div className="flex h-10 shrink-0 items-center gap-2 border-b px-3">
      <nav className="flex min-w-0 items-center gap-1 text-sm text-muted-foreground">
        {props.segments.map((s, i) => (
          <span key={i} className="flex min-w-0 items-center gap-1">
            {i > 0 && <ChevronRight className="size-3.5 shrink-0 opacity-60" />}
            <span className={i === props.segments.length - 1 ? "truncate font-medium text-foreground" : "truncate"}>
              {s}
            </span>
          </span>
        ))}
      </nav>
      <div className="ml-auto flex shrink-0 items-center gap-0.5">{props.actions}</div>
    </div>
  )
}

function IconButton(props: {
  label: string
  onClick: () => void
  disabled?: boolean
  children: React.ReactNode
}) {
  return (
    <Tooltip>
      <TooltipTrigger
        render={
          <Button
            variant="ghost"
            size="icon-sm"
            onClick={props.onClick}
            disabled={props.disabled}
            aria-label={props.label}
          >
            {props.children}
          </Button>
        }
      />
      <TooltipContent>{props.label}</TooltipContent>
    </Tooltip>
  )
}

function CopyButton({ text }: { text: string }) {
  const [copied, setCopied] = useState(false)
  const copy = async () => {
    await navigator.clipboard.writeText(text)
    setCopied(true)
    setTimeout(() => setCopied(false), 1200)
  }
  return (
    <IconButton label={copied ? "Copied" : "Copy"} onClick={copy} disabled={!text}>
      {copied ? <Check /> : <Copy />}
    </IconButton>
  )
}

function Centered({ children }: { children: React.ReactNode }) {
  return (
    <div className="flex h-dvh items-center justify-center gap-2 text-sm text-muted-foreground">{children}</div>
  )
}

function Pane(props: { title: string; meta?: string; action?: React.ReactNode; children: React.ReactNode }) {
  return (
    <div className="flex h-full flex-col">
      <div className="flex h-10 shrink-0 items-center gap-2 px-3">
        <span className="text-xs font-medium tracking-wide text-muted-foreground uppercase">{props.title}</span>
        {props.meta && <span className="font-mono text-xs text-muted-foreground/70">{props.meta}</span>}
        <div className="ml-auto">{props.action}</div>
      </div>
      <ScrollArea className="min-h-0 flex-1">
        <div className="px-2 pb-2">{props.children}</div>
      </ScrollArea>
    </div>
  )
}

function Stats({ result }: { result: RunResult }) {
  return (
    <span className="truncate text-sm text-muted-foreground">
      {result.files
        ? `Parsed ${result.files} file${result.files === 1 ? "" : "s"} in ${result.ms.toFixed(1)} ms`
        : "No files selected"}
    </span>
  )
}

function GitHubIcon() {
  return (
    <svg viewBox="0 0 16 16" aria-hidden="true" fill="currentColor">
      <path d="M8 0C3.58 0 0 3.58 0 8c0 3.54 2.29 6.53 5.47 7.59.4.07.55-.17.55-.38 0-.19-.01-.82-.01-1.49-2.01.37-2.53-.49-2.69-.94-.09-.23-.48-.94-.82-1.13-.28-.15-.68-.52-.01-.53.63-.01 1.08.58 1.23.82.72 1.21 1.87.87 2.33.66.07-.52.28-.87.51-1.07-1.78-.2-3.64-.89-3.64-3.95 0-.87.31-1.59.82-2.15-.08-.2-.36-1.02.08-2.12 0 0 .67-.21 2.2.82.64-.18 1.32-.27 2-.27.68 0 1.36.09 2 .27 1.53-1.04 2.2-.82 2.2-.82.44 1.1.16 1.92.08 2.12.51.56.82 1.27.82 2.15 0 3.07-1.87 3.75-3.65 3.95.29.25.54.73.54 1.48 0 1.07-.01 1.93-.01 2.2 0 .21.15.46.55.38A8.013 8.013 0 0016 8c0-4.42-3.58-8-8-8z" />
    </svg>
  )
}
