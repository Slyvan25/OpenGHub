/** Transient interface state: the nav drawer, dashboard layout and toasts. */

export type DashboardView = "grid" | "list";

let toastId = 0;

export interface Toast {
  id: number;
  message: string;
  tone: "info" | "error" | "success";
}

const VIEW_KEY = "openghub.view";

class UiStore {
  navOpen = $state(false);
  profilePickerOpen = $state(false);
  view = $state<DashboardView>("grid");
  toasts = $state<Toast[]>([]);

  restore() {
    if (typeof localStorage === "undefined") return;
    const stored = localStorage.getItem(VIEW_KEY);
    if (stored === "grid" || stored === "list") this.view = stored;
  }

  setView(view: DashboardView) {
    this.view = view;
    if (typeof localStorage !== "undefined") localStorage.setItem(VIEW_KEY, view);
  }

  toast(message: string, tone: Toast["tone"] = "info", ttl = 4000) {
    const toast: Toast = { id: ++toastId, message, tone };
    this.toasts = [...this.toasts, toast];
    setTimeout(() => this.dismiss(toast.id), ttl);
  }

  dismiss(id: number) {
    this.toasts = this.toasts.filter((t) => t.id !== id);
  }
}

export const ui = new UiStore();
