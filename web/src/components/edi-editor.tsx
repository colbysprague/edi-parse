import { useMemo } from "react"

import { highlightEdiRaw } from "@/lib/edi"

type Props = {
  value: string
  onChange: (value: string) => void
  placeholder?: string
}

/**
 * A plain textarea layered over a highlighted copy of the same text. Both sit
 * in one grid cell inside a single scroll container, so they always line up.
 */
export function EdiEditor({ value, onChange, placeholder }: Props) {
  const html = useMemo(() => highlightEdiRaw(value), [value])
  const lines = useMemo(() => value.split("\n").length, [value])

  return (
    <div className="h-full overflow-auto">
      <div className="flex min-h-full w-max min-w-full py-3 font-mono text-[13px] leading-6">
        <div
          aria-hidden="true"
          className="sticky left-0 z-10 w-14 shrink-0 bg-background pr-4 text-right text-muted-foreground/50 tabular-nums select-none"
        >
          {Array.from({ length: lines }, (_, i) => (
            <div key={i}>{i + 1}</div>
          ))}
        </div>
        <div className="grid flex-1">
          <pre
            aria-hidden="true"
            className="pointer-events-none col-start-1 row-start-1 m-0 pr-6 font-[inherit] whitespace-pre"
            // A trailing newline needs a character after it to take up a line.
            dangerouslySetInnerHTML={{ __html: html + (value.endsWith("\n") ? " " : "") }}
          />
          <textarea
            value={value}
            onChange={(e) => onChange(e.target.value)}
            placeholder={placeholder}
            spellCheck={false}
            autoCapitalize="off"
            autoComplete="off"
            autoCorrect="off"
            wrap="off"
            aria-label="EDI input"
            className="col-start-1 row-start-1 m-0 resize-none overflow-hidden border-0 bg-transparent p-0 pr-6 font-[inherit] leading-[inherit] whitespace-pre text-transparent caret-foreground outline-none selection:bg-primary/20 placeholder:text-muted-foreground"
          />
        </div>
      </div>
    </div>
  )
}
