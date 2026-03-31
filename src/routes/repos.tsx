import { createSignal, onMount, For, Show } from "solid-js";
import { useNavigate } from "@solidjs/router";
import { Button } from "~/components/ui/Button";
import { Input } from "~/components/ui/Input";
import { Badge } from "~/components/ui/Badge";
import { Spinner } from "~/components/ui/Spinner";
import {
  getSavedRepos,
  saveRepo,
  removeRepo,
  listUserRepos,
  searchRepos,
  logout,
} from "~/api";
import type { RepoInfo } from "~/api";
import { authStore, reposStore, uiStore } from "~/stores";
import type { RepoEntry } from "~/stores/repos.store";
import { relativeTime } from "~/lib/time";

type Tab = "saved" | "browse";

export default function ReposPage() {
  const navigate = useNavigate();
  const [tab, setTab] = createSignal<Tab>("saved");
  const [repoInput, setRepoInput] = createSignal("");
  const [inputError, setInputError] = createSignal("");

  // Browse state
  const [browseRepos, setBrowseRepos] = createSignal<RepoInfo[]>([]);
  const [browseLoading, setBrowseLoading] = createSignal(false);
  const [browseSearch, setBrowseSearch] = createSignal("");
  const [browsePage, setBrowsePage] = createSignal(1);
  const [browseHasNext, setBrowseHasNext] = createSignal(false);

  let searchTimeout: ReturnType<typeof setTimeout> | undefined;

  onMount(async () => {
    reposStore.setIsLoading(true);
    try {
      const repos = await getSavedRepos();
      reposStore.setSavedRepos(
        repos.map(([owner, repo]) => ({ owner, repo })),
      );
    } catch {
      uiStore.showError("Failed to load saved repos");
    } finally {
      reposStore.setIsLoading(false);
    }
  });

  const loadBrowseRepos = async (page: number, append = false) => {
    setBrowseLoading(true);
    try {
      const query = browseSearch().trim();
      const result = query
        ? await searchRepos(query, page, 20)
        : await listUserRepos(page, 30);

      if (append) {
        setBrowseRepos((prev) => [...prev, ...result.items]);
      } else {
        setBrowseRepos(result.items);
      }
      setBrowsePage(page);
      setBrowseHasNext(result.has_next_page);
    } catch (err) {
      uiStore.showError(`Failed to load repos: ${err}`);
    } finally {
      setBrowseLoading(false);
    }
  };

  const handleTabChange = (t: Tab) => {
    setTab(t);
    if (t === "browse" && browseRepos().length === 0) {
      loadBrowseRepos(1);
    }
  };

  const handleBrowseSearch = (value: string) => {
    setBrowseSearch(value);
    clearTimeout(searchTimeout);
    searchTimeout = setTimeout(() => {
      loadBrowseRepos(1);
    }, 300);
  };

  const handleAddRepo = async (e: Event) => {
    e.preventDefault();
    const value = repoInput().trim();
    setInputError("");

    const match = value.match(
      /^(?:https?:\/\/github\.com\/)?([^/\s]+)\/([^/\s]+?)\/?$/,
    );
    if (!match) {
      setInputError("Enter as owner/repo (e.g. facebook/react)");
      return;
    }

    const [, owner, repo] = match;
    await addRepoToSaved(owner, repo);
    setRepoInput("");
  };

  const addRepoToSaved = async (owner: string, repo: string) => {
    try {
      await saveRepo(owner, repo);
      reposStore.setSavedRepos([
        { owner, repo },
        ...reposStore.savedRepos.filter(
          (r) => !(r.owner === owner && r.repo === repo),
        ),
      ]);
    } catch {
      uiStore.showError("Failed to save repo");
    }
  };

  const handleSelectRepo = (entry: RepoEntry) => {
    reposStore.setCurrentRepo(entry);
    navigate(`/repos/${entry.owner}/${entry.repo}/pulls`);
  };

  const handleBrowseSelect = async (repo: RepoInfo) => {
    await addRepoToSaved(repo.owner, repo.name);
    reposStore.setCurrentRepo({ owner: repo.owner, repo: repo.name });
    navigate(`/repos/${repo.owner}/${repo.name}/pulls`);
  };

  const handleRemoveRepo = async (entry: RepoEntry, e: Event) => {
    e.stopPropagation();
    try {
      await removeRepo(entry.owner, entry.repo);
      reposStore.setSavedRepos(
        reposStore.savedRepos.filter(
          (r) => !(r.owner === entry.owner && r.repo === entry.repo),
        ),
      );
    } catch {
      uiStore.showError("Failed to remove repo");
    }
  };

  const handleLogout = async () => {
    await logout();
    authStore.setUser(null);
    navigate("/auth", { replace: true });
  };

  const isSaved = (owner: string, name: string) =>
    reposStore.savedRepos.some((r) => r.owner === owner && r.repo === name);

  return (
    <div class="h-full flex flex-col">
      {/* Header */}
      <div class="flex items-center justify-between px-4 py-3 border-b border-(--color-border-muted) bg-(--color-bg-secondary) shrink-0">
        <div class="flex items-center gap-3">
          <h2 class="text-sm font-semibold text-(--color-text-primary)">
            Repositories
          </h2>
          {/* Tabs */}
          <div class="flex gap-0.5 bg-(--color-bg-tertiary) rounded p-0.5">
            <button
              class={`px-3 py-1 text-xs rounded cursor-pointer transition-colors ${
                tab() === "saved"
                  ? "bg-(--color-bg-secondary) text-(--color-text-primary) shadow-sm"
                  : "text-(--color-text-secondary) hover:text-(--color-text-primary)"
              }`}
              onClick={() => handleTabChange("saved")}
            >
              Saved
            </button>
            <button
              class={`px-3 py-1 text-xs rounded cursor-pointer transition-colors ${
                tab() === "browse"
                  ? "bg-(--color-bg-secondary) text-(--color-text-primary) shadow-sm"
                  : "text-(--color-text-secondary) hover:text-(--color-text-primary)"
              }`}
              onClick={() => handleTabChange("browse")}
            >
              Browse
            </button>
          </div>
        </div>
        <Button variant="ghost" size="sm" onClick={handleLogout}>
          Sign out
        </Button>
      </div>

      {/* Content */}
      <div class="flex-1 overflow-auto p-4">
        <div class="max-w-2xl mx-auto">
          {/* Saved tab */}
          <Show when={tab() === "saved"}>
            {/* Add repo form */}
            <form onSubmit={handleAddRepo} class="flex gap-2 mb-6">
              <div class="flex-1">
                <Input
                  placeholder="owner/repo or GitHub URL"
                  value={repoInput()}
                  onInput={(e) => setRepoInput(e.currentTarget.value)}
                  error={inputError()}
                />
              </div>
              <Button type="submit" variant="primary" size="md">
                Add
              </Button>
            </form>

            <Show
              when={reposStore.savedRepos.length > 0}
              fallback={
                <div class="text-center py-12 text-(--color-text-muted) text-sm">
                  No repositories saved yet. Add one above or browse your repos.
                </div>
              }
            >
              <div class="flex flex-col gap-1">
                <For each={reposStore.savedRepos}>
                  {(entry) => (
                    <div
                      class="flex items-center justify-between w-full px-3 py-2.5 rounded-md text-left hover:bg-(--color-bg-tertiary) transition-colors group cursor-pointer"
                      onClick={() => handleSelectRepo(entry)}
                      role="button"
                      tabIndex={0}
                    >
                      <span class="text-sm text-(--color-text-primary)">
                        <span class="text-(--color-text-secondary)">
                          {entry.owner}
                        </span>
                        <span class="text-(--color-text-muted) mx-0.5">/</span>
                        <span class="font-medium">{entry.repo}</span>
                      </span>
                      <button
                        class="text-xs text-(--color-text-muted) hover:text-(--color-danger-fg) opacity-0 group-hover:opacity-100 transition-opacity cursor-pointer"
                        onClick={(e) => handleRemoveRepo(entry, e)}
                      >
                        Remove
                      </button>
                    </div>
                  )}
                </For>
              </div>
            </Show>
          </Show>

          {/* Browse tab */}
          <Show when={tab() === "browse"}>
            <div class="mb-4">
              <Input
                placeholder="Search repositories..."
                value={browseSearch()}
                onInput={(e) => handleBrowseSearch(e.currentTarget.value)}
              />
            </div>

            <Show
              when={!browseLoading() || browseRepos().length > 0}
              fallback={
                <div class="flex justify-center py-12">
                  <Spinner size={24} class="text-(--color-text-muted)" />
                </div>
              }
            >
              <div class="flex flex-col gap-1">
                <For each={browseRepos()}>
                  {(repo) => (
                    <div
                      class="flex items-center gap-3 px-3 py-2.5 rounded-md hover:bg-(--color-bg-tertiary) transition-colors cursor-pointer group"
                      onClick={() => handleBrowseSelect(repo)}
                      role="button"
                      tabIndex={0}
                    >
                      <div class="flex-1 min-w-0">
                        <div class="flex items-center gap-2">
                          <span class="text-sm font-medium text-(--color-text-primary) truncate">
                            {repo.full_name}
                          </span>
                          <Show when={repo.private}>
                            <Badge variant="warning" class="shrink-0">Private</Badge>
                          </Show>
                          <Show when={repo.fork}>
                            <Badge class="shrink-0">Fork</Badge>
                          </Show>
                          <Show when={isSaved(repo.owner, repo.name)}>
                            <Badge variant="success" class="shrink-0">Saved</Badge>
                          </Show>
                        </div>
                        <Show when={repo.description}>
                          <p class="text-xs text-(--color-text-muted) truncate mt-0.5">
                            {repo.description}
                          </p>
                        </Show>
                      </div>
                      <span class="text-xs text-(--color-text-muted) shrink-0">
                        {relativeTime(repo.updated_at)}
                      </span>
                    </div>
                  )}
                </For>
              </div>

              {/* Load more */}
              <Show when={browseHasNext()}>
                <div class="flex justify-center py-4">
                  <Button
                    variant="secondary"
                    size="sm"
                    loading={browseLoading()}
                    onClick={() => loadBrowseRepos(browsePage() + 1, true)}
                  >
                    Load more
                  </Button>
                </div>
              </Show>
            </Show>
          </Show>
        </div>
      </div>
    </div>
  );
}
