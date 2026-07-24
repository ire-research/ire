import { create } from "zustand";

interface FeedbackModalStore {
  open: boolean;
  prefill: string;
  openWith: (prefill?: string) => void;
  close: () => void;
}

export const useFeedbackModal = create<FeedbackModalStore>((set) => ({
  open: false,
  prefill: "",
  openWith: (prefill = "") => set({ open: true, prefill }),
  close: () => set({ open: false, prefill: "" }),
}));
