import { create } from "zustand";
import { persist } from "zustand/middleware";
import type { SshHost, SshKeyInfo, KnownHostEntry, TerminalInfo } from "../lib/tauri";

interface SshConfigSlice {
  hosts: SshHost[];
  loading: boolean;
  setHosts: (hosts: SshHost[]) => void;
  setLoading: (v: boolean) => void;
}

interface SshKeysSlice {
  keys: SshKeyInfo[];
  keysLoading: boolean;
  setKeys: (keys: SshKeyInfo[]) => void;
  setKeysLoading: (v: boolean) => void;
}

interface KnownHostsSlice {
  knownHosts: KnownHostEntry[];
  knownHostsLoading: boolean;
  setKnownHosts: (entries: KnownHostEntry[]) => void;
  setKnownHostsLoading: (v: boolean) => void;
}

interface LauncherSlice {
  terminals: TerminalInfo[];
  setTerminals: (terminals: TerminalInfo[]) => void;
}

interface UiSlice {
  activeTab: string;
  setActiveTab: (tab: string) => void;
  toast: { message: string; type: "success" | "error" | "info" } | null;
  showToast: (message: string, type?: "success" | "error" | "info") => void;
  clearToast: () => void;
  connectionHistory: Record<string, number>;
  recordConnection: (alias: string) => void;
}

export const useStore = create<
  SshConfigSlice & SshKeysSlice & KnownHostsSlice & LauncherSlice & UiSlice
>()(
  persist(
    (set) => ({
      // SSH Config
      hosts: [],
      loading: false,
      setHosts: (hosts) => set({ hosts }),
      setLoading: (loading) => set({ loading }),

      // SSH Keys
      keys: [],
      keysLoading: false,
      setKeys: (keys) => set({ keys }),
      setKeysLoading: (keysLoading) => set({ keysLoading }),

      // Known Hosts
      knownHosts: [],
      knownHostsLoading: false,
      setKnownHosts: (knownHosts) => set({ knownHosts }),
      setKnownHostsLoading: (knownHostsLoading) => set({ knownHostsLoading }),

      // Launcher
      terminals: [],
      setTerminals: (terminals) => set({ terminals }),

      // UI
      activeTab: "ssh-config",
      setActiveTab: (activeTab) => set({ activeTab }),
      toast: null,
      showToast: (message, type = "info") => set({ toast: { message, type } }),
      clearToast: () => set({ toast: null }),
      connectionHistory: {},
      recordConnection: (alias) =>
        set((state) => ({
          connectionHistory: { ...state.connectionHistory, [alias]: Date.now() },
        })),
    }),
    {
      name: "ssh-gui-storage",
      partialize: (state) => ({ activeTab: state.activeTab, connectionHistory: state.connectionHistory }),
    }
  )
);
