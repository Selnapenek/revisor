import { createEffect, createSignal, onMount, Show, For, onCleanup } from "solid-js";
import { useParams, useNavigate } from "@solidjs/router";
import { createVirtualizer } from "@tanstack/solid-virtual";
import { listPullRequests, searchPullRequests } from "~/api";
import { prListStore, reposStore, uiStore, prFiltersStore } from "~/stores";
import type { PrFilter } from "~/stores/pr-list.store";
import { PRListItem } from "~/components/pr-list/PRListItem";
import { prefetch, cancelPrefetch } from "~/lib/prefetch-cache";
import { PRListFilters } from "~/components/pr-list/PRListFilters";
import { PRListSearch } from "~/components/pr-list/PRListSearch";
import { PRListAdvancedFilters } from "~/components/pr-list/PRListAdvancedFilters";
import { Button } from "~/components/ui/Button";
import { Spinner } from "~/components/ui/Spinner";

export default function PRListPage() {
  const params = useParams<{ owner: string; repo: string }>();
  const navigate = useNavigate();
  let scrollRef!: HTMLDivElement;

  const currentKey = `${params.owner}/${params.repo}`;
  const hasCachedData = prListStore.repoKey === currentKey && prListStore.prs.length > 0;

  const [showLoader, setShowLoader] = createSignal(!hasCachedData);
  const [isRefreshing, setIsRefreshing] = createSignal(false);

  let initialized = false;

  const fetchPrs = async (pageNum: number, append = false, silent = false) => {
    if (!append) {
      if (!silent) setShowLoader(true);
      else setIsRefreshing(true);
    } else {
      prListStore.setIsLoadingMore(true);
    }

    try {
      const result = prFiltersStore.hasActiveFilters
        ? await searchPullRequests(
            params.owner,
            params.repo,
            prListStore.filter,
            prFiltersStore.activeFilters,
            pageNum,
            30,
          )
        : await listPullRequests(
            params.owner,
            params.repo,
            prListStore.filter,
            pageNum,
            30,
          );

      if (append) {
        prListStore.appendPrs(result.items);
      } else {
        prListStore.setPrs(result.items);
      }
      prListStore.setPage(pageNum);
      prListStore.setHasNextPage(result.has_next_page);

      prFiltersStore.updateKnownUsersFromPrs(result.items);
    } catch (err) {
      uiStore.showError(`Failed to load PRs: ${err}`);
    } finally {
      setShowLoader(false);
      setIsRefreshing(false);
      prListStore.setIsLoadingMore(false);
    }
  };

  onMount(() => {
    reposStore.setCurrentRepo({ owner: params.owner, repo: params.repo });

    prListStore.setRepoKey(currentKey);

    if (hasCachedData) {
      fetchPrs(1, false, true);
    } else {
      prListStore.clearAll();
      fetchPrs(1);
    }

    prFiltersStore.fetchLabels(params.owner, params.repo);
    initialized = true;
  });

  let alive = true;
  onCleanup(() => {
    alive = false;
    prFiltersStore.reset();
    clearTimeout(filterDebounce);
  });

  createEffect(() => {
    const _filter = prListStore.filter;
    if (!initialized) return;
    fetchPrs(1);
  });

  let filterDebounce: ReturnType<typeof setTimeout> | undefined;
  createEffect(() => {
    const _filters = JSON.stringify(prFiltersStore.activeFilters);
    if (!initialized) return;
    clearTimeout(filterDebounce);
    filterDebounce = setTimeout(() => {
      if (!alive) return;
      fetchPrs(1);
    }, 300);
  });

  const loadMore = () => {
    if (prListStore.hasNextPage && !prListStore.isLoadingMore) {
      fetchPrs(prListStore.page + 1, true);
    }
  };

  const handleFilterChange = (f: PrFilter) => {
    prListStore.setFilter(f);
  };

  const handlePrClick = (number: number) => {
    navigate(`/repos/${params.owner}/${params.repo}/pulls/${number}`);
  };

  const handlePrHover = (number: number) => {
    prefetch(params.owner, params.repo, number);
  };

  // Virtual scrolling
  const items = () => prListStore.filteredPrs;

  const virtualizer = createVirtualizer({
    get count() { return items().length; },
    getScrollElement: () => scrollRef,
    estimateSize: () => 52,
    overscan: 15,
  });

  // Infinite scroll: load more when near bottom
  const handleScroll = () => {
    if (!scrollRef) return;
    const { scrollTop, scrollHeight, clientHeight } = scrollRef;
    if (scrollHeight - scrollTop - clientHeight < 200) {
      loadMore();
    }
  };

  return (
    <div class="h-full flex flex-col">
      {/* Header */}
      <div class="flex items-center gap-3 px-4 py-3 border-b border-(--color-border-muted) bg-(--color-bg-secondary) shrink-0">
        <Button variant="ghost" size="sm" onClick={() => navigate("/repos")}>
          &larr; Repos
        </Button>
        <span class="text-sm text-(--color-text-secondary)">
          {params.owner}
          <span class="text-(--color-text-muted) mx-0.5">/</span>
          <span class="font-semibold text-(--color-text-primary)">{params.repo}</span>
        </span>
        <div class="ml-auto flex items-center gap-3">
          <PRListSearch
            value={prListStore.searchQuery}
            onChange={(v) => prListStore.setSearchQuery(v)}
          />
          <PRListFilters
            current={prListStore.filter}
            onChange={handleFilterChange}
          />
        </div>
      </div>

      {/* Advanced filters */}
      <PRListAdvancedFilters />

      {/* PR list */}
      <div class="flex-1 relative overflow-hidden">
        {/* Spinner overlay — only when loading with no data */}
        <Show when={showLoader() && items().length === 0}>
          <div class="absolute inset-0 flex items-center justify-center bg-(--color-bg-primary) z-10">
            <Spinner size={24} class="text-(--color-text-muted)" />
          </div>
        </Show>

        {/* Empty state */}
        <Show when={!showLoader() && items().length === 0}>
          <div class="absolute inset-0 flex flex-col items-center justify-center gap-2 text-sm text-(--color-text-muted)">
            <span>{prFiltersStore.hasActiveFilters ? "No pull requests match the active filters" : "No pull requests found"}</span>
            <Show when={prFiltersStore.hasActiveFilters}>
              <button
                class="text-xs text-(--color-accent-fg) hover:underline cursor-pointer"
                onClick={() => prFiltersStore.clearAll()}
              >
                Clear filters
              </button>
            </Show>
          </div>
        </Show>

        {/* Scroll container — always in DOM so virtualizer keeps its ref */}
        <div
          ref={scrollRef!}
          class="h-full overflow-auto"
          onScroll={handleScroll}
          onMouseLeave={cancelPrefetch}
        >
          <Show when={isRefreshing()}>
            <div class="flex items-center justify-center gap-2 px-4 py-1.5 text-xs text-(--color-text-muted)">
              <Spinner size={12} class="text-(--color-text-muted)" />
              <span>Refreshing…</span>
            </div>
          </Show>

          <div
            style={{
              height: `${virtualizer.getTotalSize()}px`,
              position: "relative",
            }}
          >
            <For each={virtualizer.getVirtualItems()}>
              {(virtualRow) => {
                const pr = () => items()[virtualRow.index];
                return (
                  <div
                    style={{
                      position: "absolute",
                      top: 0,
                      left: 0,
                      width: "100%",
                      transform: `translateY(${virtualRow.start}px)`,
                    }}
                    data-index={virtualRow.index}
                    ref={(el) => virtualizer.measureElement(el)}
                  >
                    <PRListItem
                      pr={pr()}
                      onClick={() => handlePrClick(pr().number)}
                      onHover={() => handlePrHover(pr().number)}
                    />
                  </div>
                );
              }}
            </For>
          </div>

          <Show when={prListStore.isLoadingMore}>
            <div class="flex justify-center py-4">
              <Spinner size={18} class="text-(--color-text-muted)" />
            </div>
          </Show>
        </div>
      </div>
    </div>
  );
}
