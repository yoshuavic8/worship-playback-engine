import { DndContext, PointerSensor, closestCenter, useSensor, useSensors, type DragEndEvent } from "@dnd-kit/core";
import { SortableContext, arrayMove, horizontalListSortingStrategy, useSortable } from "@dnd-kit/sortable";
import { CSS } from "@dnd-kit/utilities";
import { Plus, RotateCcw, X } from "lucide-react";
import { useMemo } from "react";
import { fmtTime } from "@/lib/format";
import { sectionColor, type ResolvedSection } from "@/lib/sections";
import { Button } from "./ui";

/** Arrangement = ordered list of section ids. Entries get a positional key so repeats can be dragged. */
export function ArrangementEditor({ sections, value, onChange }: {
  sections: ResolvedSection[]; value: string[]; onChange: (v: string[]) => void;
}) {
  const sensors = useSensors(useSensor(PointerSensor, { activationConstraint: { distance: 4 } }));
  const natural = value.length === 0;
  const order = natural ? sections.map((s) => s.id) : value;
  const items = useMemo(() => order.map((id, i) => ({ key: `${i}:${id}`, id })), [order]);
  const byId = (id: string) => sections.find((s) => s.id === id);
  const total = order.reduce((a, id) => a + ((byId(id)?.end ?? 0) - (byId(id)?.start ?? 0)), 0);

  const onDragEnd = (e: DragEndEvent) => {
    if (!e.over || e.active.id === e.over.id) return;
    const from = items.findIndex((x) => x.key === e.active.id);
    const to = items.findIndex((x) => x.key === e.over!.id);
    onChange(arrayMove(order, from, to));
  };

  return (
    <div>
      <div className="flex min-h-[3.25rem] flex-wrap items-center gap-1.5 rounded-xl border border-line bg-bg-2/60 p-2">
        <DndContext sensors={sensors} collisionDetection={closestCenter} onDragEnd={onDragEnd}>
          <SortableContext items={items.map((x) => x.key)} strategy={horizontalListSortingStrategy}>
            {items.map((it, i) => {
              const s = byId(it.id);
              if (!s) return null;
              return <Chip key={it.key} id={it.key} section={s} onRemove={() => onChange(order.filter((_, j) => j !== i))} />;
            })}
          </SortableContext>
        </DndContext>
        {order.length === 0 && <span className="px-2 text-sm text-muted">Add sections below</span>}
      </div>
      <div className="mt-2 flex items-center justify-between text-xs text-muted">
        <span>{natural ? "Natural order (as recorded)" : `Custom arrangement · ${order.length} parts`} · {fmtTime(total)}</span>
        {!natural && <Button size="sm" variant="ghost" icon={<RotateCcw className="size-3.5" />} onClick={() => onChange([])}>Natural order</Button>}
      </div>
      <div className="mt-3">
        <div className="label mb-1.5">Add section to arrangement</div>
        <div className="flex flex-wrap gap-1.5">
          {sections.filter((s) => s.id !== "__song").map((s) => {
            const dur = s.end - s.start;
            return (
              <button key={s.id} onClick={() => onChange([...order, s.id])}
                className="flex h-8 items-center gap-1.5 rounded-md border border-line px-2 text-xs font-medium text-fg-2 hover:border-line-2 hover:text-fg">
                <Plus className="size-3" />
                <span className="size-2 rounded-full" style={{ background: sectionColor(s.kind) }} />
                {s.label}
                <span className="text-faint">{fmtTime(dur)}</span>
              </button>
            );
          })}
        </div>
      </div>
    </div>
  );
}

function Chip({ id, section, onRemove }: { id: string; section: ResolvedSection; onRemove: () => void }) {
  const { attributes, listeners, setNodeRef, transform, transition, isDragging } = useSortable({ id });
  const color = sectionColor(section.kind);
  const dur = section.end - section.start;
  return (
    <div
      ref={setNodeRef}
      style={{ transform: CSS.Transform.toString(transform), transition, borderColor: `color-mix(in oklab, ${color} 45%, transparent)`, background: `color-mix(in oklab, ${color} 12%, transparent)`, zIndex: isDragging ? 10 : undefined }}
      className="group flex h-9 cursor-grab items-center gap-1 rounded-lg border pr-1 pl-2.5 text-xs font-semibold active:cursor-grabbing"
      {...attributes}
      {...listeners}
    >
      <div className="flex flex-col leading-none">
        <span style={{ color }}>{section.label}</span>
        <span className="tnum mt-0.5 text-[0.6rem] font-normal opacity-50">{fmtTime(dur)}</span>
      </div>
      <button onPointerDown={(e) => e.stopPropagation()} onClick={onRemove} className="grid size-5 place-items-center rounded text-muted opacity-0 group-hover:opacity-100 hover:bg-white/10 hover:text-fg" aria-label={`Remove ${section.label}`}>
        <X className="size-3" />
      </button>
    </div>
  );
}
