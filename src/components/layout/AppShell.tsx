import { type JSX, Show } from "solid-js";
import { authStore } from "~/stores";
import { uiStore } from "~/stores";
import { StatusBar } from "./StatusBar";

interface AppShellProps {
  children: JSX.Element;
}

export function AppShell(props: AppShellProps) {
  return (
    <div class="h-full flex flex-col bg-(--color-bg-primary)">
      {/* Error toast */}
      <Show when={uiStore.error}>
        <div class="fixed top-4 right-4 z-50 max-w-md px-4 py-3 rounded-lg bg-(--color-danger-emphasis) text-white text-sm shadow-lg flex items-center gap-2">
          <span class="flex-1">{uiStore.error}</span>
          <button
            class="text-white/70 hover:text-white cursor-pointer"
            onClick={() => uiStore.clearError()}
          >
            &times;
          </button>
        </div>
      </Show>

      {/* Content */}
      <div class="flex-1 overflow-hidden">{props.children}</div>

      {/* Status bar */}
      <Show when={authStore.isAuthenticated}>
        <StatusBar />
      </Show>
    </div>
  );
}
