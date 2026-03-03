import { DndContext, closestCenter } from "@dnd-kit/core";
import type { DragEndEvent } from "@dnd-kit/core";
import { SortableContext, useSortable, verticalListSortingStrategy, arrayMove } from "@dnd-kit/sortable";
import { CSS } from "@dnd-kit/utilities";
import type { SshHost } from "../../lib/tauri";
import { HostCard } from "./HostCard";

interface SortableHostCardProps {
  host: SshHost;
  lastConnected?: number;
  onEdit: () => void;
  onDelete: () => void;
  onConnect: () => void;
  onDuplicate: () => void;
}

function SortableHostCard({ host, lastConnected, onEdit, onDelete, onConnect, onDuplicate }: SortableHostCardProps) {
  const { attributes, listeners, setNodeRef, transform, transition, isDragging } = useSortable({ id: host.alias });
  return (
    <div
      ref={setNodeRef}
      style={{ transform: CSS.Transform.toString(transform), transition, opacity: isDragging ? 0.5 : 1 }}
    >
      <HostCard
        host={host}
        lastConnected={lastConnected}
        onEdit={onEdit}
        onDelete={onDelete}
        onConnect={onConnect}
        onDuplicate={onDuplicate}
        dragHandleProps={{ ...attributes, ...listeners }}
      />
    </div>
  );
}

interface HostListProps {
  hosts: SshHost[];
  connectionHistory: Record<string, number>;
  onEdit: (host: SshHost) => void;
  onDelete: (alias: string) => void;
  onReorder: (reordered: SshHost[]) => void;
  onConnect: (host: SshHost) => void;
  onDuplicate: (host: SshHost) => void;
}

export function HostList({ hosts, connectionHistory, onEdit, onDelete, onReorder, onConnect, onDuplicate }: HostListProps) {
  function handleDragEnd(event: DragEndEvent) {
    const { active, over } = event;
    if (over && active.id !== over.id) {
      const oldIdx = hosts.findIndex((h) => h.alias === active.id);
      const newIdx = hosts.findIndex((h) => h.alias === over.id);
      onReorder(arrayMove(hosts, oldIdx, newIdx));
    }
  }

  return (
    <DndContext collisionDetection={closestCenter} onDragEnd={handleDragEnd}>
      <SortableContext items={hosts.map((h) => h.alias)} strategy={verticalListSortingStrategy}>
        <div className="flex flex-col gap-2">
          {hosts.map((host) => (
            <SortableHostCard
              key={host.alias}
              host={host}
              lastConnected={connectionHistory[host.alias]}
              onEdit={() => onEdit(host)}
              onDelete={() => onDelete(host.alias)}
              onConnect={() => onConnect(host)}
              onDuplicate={() => onDuplicate(host)}
            />
          ))}
        </div>
      </SortableContext>
    </DndContext>
  );
}
