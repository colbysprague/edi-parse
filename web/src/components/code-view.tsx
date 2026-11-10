import { useMemo } from "react"

import { ScrollArea, ScrollBar } from "@/components/ui/scroll-area"
import { highlightEdi, highlightJson } from "@/lib/edi"

type Props = { language: "json" | "edi"; text: string }

export function CodeView({ language, text }: Props) {
  const lines = useMemo(
    () => (language === "json" ? highlightJson(text) : highlightEdi(text)),
    [language, text],
  )

  return (
    <ScrollArea className="h-full min-h-0">
      <pre className="w-max min-w-full py-3 font-mono text-[13px] leading-6">
        {lines.map((html, i) => (
          <div key={i} className="flex">
            <span className="sticky left-0 w-14 shrink-0 bg-background pr-4 text-right text-muted-foreground/50 tabular-nums select-none">
              {i + 1}
            </span>
            <span className="pr-6 whitespace-pre" dangerouslySetInnerHTML={{ __html: html }} />
          </div>
        ))}
      </pre>
      <ScrollBar orientation="horizontal" />
    </ScrollArea>
  )
}
