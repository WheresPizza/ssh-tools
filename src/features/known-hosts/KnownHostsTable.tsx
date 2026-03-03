import type { KnownHostEntry } from "../../lib/tauri";

type VerifyState = 'checking' | 'match' | 'mismatch' | 'error';

interface KnownHostsTableProps {
  entries: KnownHostEntry[];
  selected: Set<number>;
  onSelect: (selected: Set<number>) => void;
  onDelete: (lineNumber: number) => void;
  verifyStatus: Record<number, VerifyState>;
  onVerify: (entry: KnownHostEntry) => void;
}

function VerifyStatusIcon({ status }: { status: VerifyState }) {
  if (status === 'checking') {
    return <span className="inline-block w-3 h-3 border-2 border-current border-t-transparent rounded-full animate-spin text-muted-foreground" />;
  }
  if (status === 'match') {
    return <span className="text-green-600 dark:text-green-400 font-bold">✓</span>;
  }
  if (status === 'mismatch') {
    return <span className="text-red-600 dark:text-red-400 font-bold">✗</span>;
  }
  return <span className="text-yellow-600 dark:text-yellow-400 font-bold">⚠</span>;
}

export function KnownHostsTable({ entries, selected, onSelect, onDelete, verifyStatus, onVerify }: KnownHostsTableProps) {
  const toggleAll = () => {
    if (selected.size === entries.length) {
      onSelect(new Set());
    } else {
      onSelect(new Set(entries.map((e) => e.line_number)));
    }
  };

  const toggleOne = (lineNumber: number) => {
    const next = new Set(selected);
    if (next.has(lineNumber)) {
      next.delete(lineNumber);
    } else {
      next.add(lineNumber);
    }
    onSelect(next);
  };

  return (
    <div className="border border-border rounded-lg overflow-hidden">
      <table className="w-full text-sm">
        <thead className="bg-muted/50">
          <tr>
            <th className="w-10 p-3 text-left">
              <input
                type="checkbox"
                checked={selected.size === entries.length && entries.length > 0}
                onChange={toggleAll}
                className="rounded"
              />
            </th>
            <th className="p-3 text-left font-medium text-muted-foreground">Hostname</th>
            <th className="p-3 text-left font-medium text-muted-foreground">Key Type</th>
            <th className="w-32 p-3" />
          </tr>
        </thead>
        <tbody>
          {entries.map((entry, i) => (
            <tr
              key={entry.line_number}
              className={`border-t border-border ${i % 2 === 0 ? "" : "bg-muted/20"} hover:bg-accent/30 transition-colors`}
            >
              <td className="p-3">
                <input
                  type="checkbox"
                  checked={selected.has(entry.line_number)}
                  onChange={() => toggleOne(entry.line_number)}
                  className="rounded"
                />
              </td>
              <td className="p-3 font-mono text-xs text-foreground break-all">
                {entry.hostname}
              </td>
              <td className="p-3 text-xs text-muted-foreground">{entry.key_type}</td>
              <td className="p-3 text-right">
                <div className="flex items-center justify-end gap-2">
                  {!entry.hostname.startsWith("|1|") && (
                    verifyStatus[entry.line_number] ? (
                      <VerifyStatusIcon status={verifyStatus[entry.line_number]} />
                    ) : (
                      <button
                        onClick={() => onVerify(entry)}
                        className="text-xs text-muted-foreground hover:text-foreground hover:underline"
                      >
                        Verify
                      </button>
                    )
                  )}
                  <button
                    onClick={() => onDelete(entry.line_number)}
                    className="text-xs text-destructive hover:underline"
                  >
                    Remove
                  </button>
                </div>
              </td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}
