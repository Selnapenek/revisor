import { createSignal } from "solid-js";

const [error, setError] = createSignal<string | null>(null);

export const uiStore = {
  get error() { return error(); },

  setError,

  showError(message: string, duration = 5000) {
    setError(message);
    setTimeout(() => setError(null), duration);
  },

  clearError() {
    setError(null);
  },
};
