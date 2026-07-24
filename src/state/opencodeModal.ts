import { create } from "zustand";

/** Shared open/close state for OpenCodeProvidersModal — it has two independent
 * entry points (the settings popover, and the composer's "Browse OpenCode
 * models…" row) that don't share a component ancestor close enough to prop-drill
 * a callback through, so this is a store instead. */
interface OpenCodeModalState {
  open: boolean;
  openModal(): void;
  closeModal(): void;
}

export const useOpenCodeModal = create<OpenCodeModalState>((set) => ({
  open: false,
  openModal: () => set({ open: true }),
  closeModal: () => set({ open: false }),
}));
