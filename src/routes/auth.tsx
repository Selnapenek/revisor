import { createSignal } from "solid-js";
import { useNavigate } from "@solidjs/router";
import { Button } from "~/components/ui/Button";
import { Input } from "~/components/ui/Input";
import { validateAndStoreToken } from "~/api";
import { authStore, uiStore } from "~/stores";

export default function AuthPage() {
  const navigate = useNavigate();
  const [token, setToken] = createSignal("");
  const [isValidating, setIsValidating] = createSignal(false);
  const [error, setError] = createSignal("");

  const handleSubmit = async (e: Event) => {
    e.preventDefault();
    const value = token().trim();
    if (!value) {
      setError("Token is required");
      return;
    }

    setIsValidating(true);
    setError("");

    try {
      const user = await validateAndStoreToken(value);
      authStore.setUser(user);
      navigate("/repos", { replace: true });
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err);
      if (message.includes("401") || message.includes("Unauthorized")) {
        setError("Invalid token. Please check and try again.");
      } else {
        setError(message);
      }
    } finally {
      setIsValidating(false);
    }
  };

  return (
    <div class="h-full flex items-center justify-center">
      <div class="w-full max-w-sm mx-auto px-6">
        <div class="text-center mb-8">
          <h1 class="text-2xl font-bold text-(--color-text-primary) mb-2">
            Revisor
          </h1>
          <p class="text-sm text-(--color-text-secondary)">
            Fast, offline-capable code review
          </p>
        </div>

        <form onSubmit={handleSubmit} class="flex flex-col gap-4">
          <div class="flex flex-col gap-2">
            <label class="text-sm font-medium text-(--color-text-secondary)">
              GitHub Personal Access Token
            </label>
            <Input
              type="password"
              placeholder="ghp_xxxxxxxxxxxxxxxxxxxx"
              value={token()}
              onInput={(e) => setToken(e.currentTarget.value)}
              error={error()}
              autofocus
            />
            <p class="text-xs text-(--color-text-muted)">
              Needs <code class="px-1 py-0.5 rounded bg-(--color-bg-tertiary)">repo</code> scope.
              Create one in GitHub Settings &rarr; Developer settings &rarr; Personal access tokens.
            </p>
          </div>

          <Button
            type="submit"
            variant="primary"
            size="lg"
            loading={isValidating()}
            class="w-full"
          >
            Connect to GitHub
          </Button>
        </form>
      </div>
    </div>
  );
}
