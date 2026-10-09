import { create } from "zustand";
import type { TerminalOperation } from "./tauri";
export const useOperations = create<{ operations: TerminalOperation[]; replace: (operations: TerminalOperation[]) => void; track: (operation: TerminalOperation) => void }>(set => ({
  operations: [],
  replace: operations => set({ operations }),
  track: operation => set(state => ({ operations: operation.id ? [...state.operations.filter(op => op.id !== operation.id), operation] : state.operations })),
}));
export const isPending = (operation: TerminalOperation) => operation.state === "started" || operation.state === "awaiting_input";
