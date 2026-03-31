import { createSignal, onMount, onCleanup, Show } from "solid-js";
import { getRateLimit } from "~/api";
import { authStore } from "~/stores";
import type { RateLimitInfo } from "~/types/pr";

export function StatusBar() {
  const [rateLimit, setRateLimit] = createSignal<RateLimitInfo | null>(null);

  const pollRateLimit = async () => {
    if (!authStore.isAuthenticated) return;
    try {
      const info = await getRateLimit();
      setRateLimit(info);
    } catch {
      // Silently ignore - rate limit is informational
    }
  };

  onMount(() => {
    pollRateLimit();
    const interval = setInterval(pollRateLimit, 30_000);
    onCleanup(() => clearInterval(interval));
  });

  const rateLimitColor = () => {
    const rl = rateLimit();
    if (!rl || rl.limit === 0) return "text-(--color-text-muted)";
    const pct = rl.remaining / rl.limit;
    if (pct < 0.1) return "text-(--color-danger-fg)";
    if (pct < 0.3) return "text-(--color-warning-fg)";
    return "text-(--color-text-muted)";
  };

  return (
    <div class="h-6 flex items-center px-3 text-xs text-(--color-text-muted) bg-(--color-bg-secondary) border-t border-(--color-border-muted) gap-4 shrink-0">
      <span>Revisor</span>

      <Show when={rateLimit()}>
        {(rl) => (
          <span class={rateLimitColor()}>
            API: {rl().remaining}/{rl().limit}
          </span>
        )}
      </Show>

      <span class="ml-auto">
        {authStore.user?.login}
      </span>
    </div>
  );
}
