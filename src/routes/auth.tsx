import { createSignal, onCleanup } from "solid-js";
import { useNavigate } from "@solidjs/router";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { open } from "@tauri-apps/plugin-shell";
import { Button } from "~/components/ui/Button";
import { startGithubLogin, waitForGithubLogin, cancelGithubLogin } from "~/api";
import { authStore } from "~/stores";

export default function AuthPage() {
  const navigate = useNavigate();
  const [attemptId, setAttemptId] = createSignal<string>();
  const [userCode, setUserCode] = createSignal("");
  const [verificationUri, setVerificationUri] = createSignal("");
  const [isStarting, setIsStarting] = createSignal(false);
  const [isWaiting, setIsWaiting] = createSignal(false);
  const [error, setError] = createSignal("");
  let closed = false;

  onCleanup(() => {
    closed = true;
    const id = attemptId();
    if (id) void cancelGithubLogin(id).catch(() => undefined);
  });

  const openGithub = async () => {
    try {
      await open(verificationUri());
      setError("");
    } catch {
      setError("Could not open your browser. Copy the address and code to sign in on GitHub.");
    }
  };

  const beginLogin = async () => {
    setIsStarting(true);
    setError("");
    let startedAttemptId: string | undefined;
    try {
      const info = await startGithubLogin();
      startedAttemptId = info.attemptId;
      if (closed) {
        await cancelGithubLogin(info.attemptId);
        return;
      }
      setAttemptId(info.attemptId);
      setUserCode(info.userCode);
      setVerificationUri(info.verificationUri);
      setIsWaiting(true);
      try {
        await open(info.verificationUri);
      } catch {
        setError("Could not open your browser. Copy the address and code to sign in on GitHub.");
      }
      const user = await waitForGithubLogin(info.attemptId);
      if (closed) return;
      authStore.setUser(user);
      try { await getCurrentWindow().setFocus(); } catch { /* Login still succeeds if focus is unavailable. */ }
      navigate("/repos", { replace: true });
    } catch (err) {
      if (!closed && attemptId() === startedAttemptId) {
        setError(err instanceof Error ? err.message : String(err));
        setAttemptId(undefined);
        setUserCode("");
        setVerificationUri("");
      }
    } finally {
      if (!closed) {
        setAttemptId(undefined);
        setIsStarting(false);
        setIsWaiting(false);
      }
    }
  };

  const cancelLogin = async () => {
    const id = attemptId();
    setAttemptId(undefined);
    setUserCode("");
    setVerificationUri("");
    setIsWaiting(false);
    setError("");
    if (id) await cancelGithubLogin(id).catch(() => undefined);
  };

  return (
    <div class="h-full flex items-center justify-center">
      <div class="w-full max-w-sm mx-auto px-6">
        <div class="text-center mb-8">
          <h1 class="text-2xl font-bold text-(--color-text-primary) mb-2">Revisor</h1>
          <p class="text-sm text-(--color-text-secondary)">Fast, offline-capable code review</p>
        </div>
        <div class="flex flex-col gap-4">
          <p class="text-sm text-(--color-text-secondary) text-center">
            Sign in with GitHub to review your repositories. Revisor needs access to your repositories, including private repositories.
          </p>
          {userCode() && (
            <div class="rounded-md border border-(--color-border-default) bg-(--color-bg-secondary) p-4 text-center">
              <p class="text-xs text-(--color-text-muted) mb-2">Enter this code on GitHub</p>
              <code class="select-all text-2xl font-bold tracking-widest text-(--color-text-primary)">{userCode()}</code>
              <p class="mt-3 text-xs text-(--color-text-muted) break-all">{verificationUri()}</p>
            </div>
          )}
          {error() && <p role="alert" class="text-sm text-(--color-danger-fg)">{error()}</p>}
          {userCode() ? (
            <>
              <Button type="button" variant="secondary" size="lg" class="w-full" onClick={openGithub}>Open GitHub again</Button>
              {isWaiting() && <p class="text-xs text-center text-(--color-text-muted)">Complete sign-in in your browser. Revisor will continue when you approve access.</p>}
              <Button type="button" variant="ghost" class="w-full" onClick={cancelLogin}>Cancel</Button>
            </>
          ) : (
            <Button type="button" variant="primary" size="lg" class="w-full" loading={isStarting()} onClick={beginLogin}>
              Sign in with GitHub
            </Button>
          )}
        </div>
      </div>
    </div>
  );
}
