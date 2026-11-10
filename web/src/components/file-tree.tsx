import { ChevronRight, FileCode2, FileJson, FileX, Folder, FolderOpen } from "lucide-react"

import { Checkbox } from "@/components/ui/checkbox"
import { leaves, type TreeNode } from "@/lib/edi"
import { cn } from "@/lib/utils"

type Props = {
  nodes: TreeNode[]
  expanded: Set<string>
  onToggleExpanded: (path: string) => void
  activePath?: string
  onOpen: (path: string) => void
  /** Omit to render without checkboxes. */
  selected?: Set<string>
  onSelect?: (paths: string[], on: boolean) => void
  failed?: Set<string>
  modified?: Set<string>
  /** Collapsed instead of expanded: `expanded` lists collapsed dirs. */
  invertExpanded?: boolean
}

export function FileTree(props: Props) {
  return (
    <ul role="tree" className="flex flex-col gap-px py-1">
      {props.nodes.map((n) => (
        <Node key={n.path} node={n} depth={0} {...props} />
      ))}
    </ul>
  )
}

function Node({ node, depth, ...p }: Props & { node: TreeNode; depth: number }) {
  const dir = node.children !== null
  const open = dir && p.expanded.has(node.path) !== !!p.invertExpanded
  const active = !dir && p.activePath === node.path
  const failed = p.failed?.has(node.path)
  const modified = !dir && p.modified?.has(node.path)

  let checkbox = null
  let count = null
  if (p.selected && p.onSelect) {
    const all = leaves(node)
    const n = all.filter((x) => p.selected!.has(x)).length
    const checked = n === all.length
    checkbox = (
      <Checkbox
        checked={checked}
        indeterminate={n > 0 && !checked}
        onCheckedChange={() => p.onSelect!(all, !checked)}
        onClick={(e) => e.stopPropagation()}
        aria-label={`Select ${node.path}`}
      />
    )
    if (dir && n > 0) {
      count = (
        <span className="ml-auto pl-2 text-xs tabular-nums text-muted-foreground">
          {n}/{all.length}
        </span>
      )
    }
  }

  const Icon = dir
    ? open
      ? FolderOpen
      : Folder
    : failed
      ? FileX
      : node.name.endsWith(".json")
        ? FileJson
        : FileCode2

  return (
    <li role="treeitem" aria-expanded={dir ? open : undefined} aria-selected={active}>
      <div
        onClick={() => (dir ? p.onToggleExpanded(node.path) : p.onOpen(node.path))}
        title={node.path}
        className={cn(
          "group flex h-7 cursor-pointer items-center gap-1.5 rounded-md pr-2 text-sm select-none",
          "text-sidebar-foreground/80 hover:bg-sidebar-accent hover:text-sidebar-accent-foreground",
          active && "bg-sidebar-accent font-medium text-sidebar-accent-foreground",
          failed && "text-destructive hover:text-destructive",
        )}
        style={{ paddingLeft: 6 + depth * 14 }}
      >
        <ChevronRight
          className={cn(
            "size-3.5 shrink-0 text-muted-foreground transition-transform",
            !dir && "invisible",
            open && "rotate-90",
          )}
        />
        {checkbox}
        <Icon
          className={cn(
            "size-4 shrink-0",
            failed ? "text-destructive" : dir ? "text-muted-foreground" : "text-muted-foreground/80",
          )}
        />
        <span className="truncate">{node.name}</span>
        {count}
        {modified && (
          <span title="Modified" className="ml-1 size-1.5 shrink-0 rounded-full bg-amber-500" />
        )}
        {failed && <span className="ml-auto pl-2 text-xs">failed</span>}
      </div>
      {dir && open && (
        <ul role="group" className="flex flex-col gap-px pt-px">
          {node.children!.map((c) => (
            <Node key={c.path} node={c} depth={depth + 1} {...p} />
          ))}
        </ul>
      )}
    </li>
  )
}
