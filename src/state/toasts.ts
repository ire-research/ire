import { create } from "zustand";

export type ToastKind = "error" | "info" | "success";

export interface Toast {
  id: string;
  kind: ToastKind;
  message: string;
  scope?: string;
  /** Skip auto-dismiss; the toast stays until the user closes it. */
  persistent?: boolean;
  /** Clickable label (e.g. a version) that opens `url` in the default browser. */
  link?: { label: string; url: string };
  /** Action button; receives this toast's id (e.g. to dismiss itself on click). */
  action?: { label: string; onClick: (id: string) => void };
}

interface ToastStore {
  toasts: Toast[];
  push: (toast: Omit<Toast, "id">) => string;
  dismiss: (id: string) => void;
}

export const useToasts = create<ToastStore>((set) => ({
  toasts: [],
  push: (toast) => {
    const id = crypto.randomUUID();
    set((s) => ({ toasts: [...s.toasts, { ...toast, id }] }));
    return id;
  },
  dismiss: (id) =>
    set((s) => ({ toasts: s.toasts.filter((t) => t.id !== id) })),
}));

/** Convenience: show an error toast for a thrown/rejected value. */
export function toastError(scope: string, err: unknown) {
  const message = err instanceof Error ? err.message : String(err);
  useToasts.getState().push({ kind: "error", scope, message });
}
