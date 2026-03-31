import { Router, Route, Navigate } from "@solidjs/router";
import { onMount, Show } from "solid-js";
import { AppShell } from "~/components/layout/AppShell";
import { Spinner } from "~/components/ui/Spinner";
import { restoreSession } from "~/api";
import { authStore } from "~/stores";
import AuthPage from "~/routes/auth";
import ReposPage from "~/routes/repos";
import PRListPage from "~/routes/pr-list";
import PRDetailPage from "~/routes/pr-detail";

function AuthGuard(props: { children: any }) {
  return (
    <Show
      when={authStore.isAuthenticated}
      fallback={<Navigate href="/auth" />}
    >
      {props.children}
    </Show>
  );
}

export default function App() {
  onMount(async () => {
    try {
      const user = await restoreSession();
      if (user) {
        authStore.setUser(user);
      }
    } catch {
      // Token invalid or network error — user will need to re-auth
    } finally {
      authStore.setIsLoading(false);
    }
  });

  return (
    <Show
      when={!authStore.isLoading}
      fallback={
        <div class="h-full flex items-center justify-center bg-(--color-bg-primary)">
          <Spinner size={32} class="text-(--color-text-muted)" />
        </div>
      }
    >
      <AppShell>
        <Router>
          <Route path="/auth" component={AuthPage} />
          <Route path="/repos" component={() => (
            <AuthGuard><ReposPage /></AuthGuard>
          )} />
          <Route path="/repos/:owner/:repo/pulls" component={() => (
            <AuthGuard><PRListPage /></AuthGuard>
          )} />
          <Route path="/repos/:owner/:repo/pulls/:number" component={() => (
            <AuthGuard><PRDetailPage /></AuthGuard>
          )} />
          <Route path="*" component={() => (
            <Show when={authStore.isAuthenticated} fallback={<Navigate href="/auth" />}>
              <Navigate href="/repos" />
            </Show>
          )} />
        </Router>
      </AppShell>
    </Show>
  );
}
