import { createSignal, createEffect, onMount, Show, For, onCleanup } from "solid-js";
import { useParams, useNavigate } from "@solidjs/router";
import { createVirtualizer } from "@tanstack/solid-virtual";
import { listPullRequests } from "~/api";
import { prListStore, reposStore, uiStore } from "~/stores";
import type { PrFilter } from "~/stores/pr-list.store";
import { PRListItem } from "~/components/pr-list/PRListItem";
import { PRListFilters } from "~/components/pr-list/PRListFilters";
import { PRListSearch } from "~/components/pr-list/PRListSearch";
import { Button } from "~/components/ui/Button";
import { Spinner } from "~/components/ui/Spinner";

export default function PRListPage() {
  const params = useParams<{ owner: string; repo: string }>();
  const navigate = useNavigate();
  let scrollRef!: HTMLDivElement;

  // Set current repo from URL params
  onMount(() => {
    reposStore.setCurrentRepo({ owner: params.owner, repo: params.repo });
  });

  const fetchPrs = async (pageNum: number, append = false) => {
    if (!append) {
      prListStore.setIsLoading(true);
    } else {
      prListStore.setIsLoadingMore(true);
    }

    try {
      const result = await listPullRequests(
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
    } catch (err) {
      uiStore.showError(`Failed to load PRs: ${err}`);
    } finally {
      prListStore.setIsLoading(false);
      prListStore.setIsLoadingMore(false);
    }
  };

  // Load PRs on mount and when filter changes
  onMount(() => fetchPrs(1));

  createEffect(() => {
    const _filter = prListStore.filter;
    prListStore.reset();
    fetchPrs(1);
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

  // Virtual scrolling
  const items = () => prListStore.filteredPrs;

  const virtualizer = createVirtualizer({
    get count() { return items().length; },
    getScrollElement: () => scrollRef,
    estimateSize: () => 64,
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

      {/* PR list */}
      <Show
        when={!prListStore.isLoading}
        fallback={
          <div class="flex-1 flex items-center justify-center">
            <Spinner size={24} class="text-(--color-text-muted)" />
          </div>
        }
      >
        <Show
          when={items().length > 0}
          fallback={
            <div class="flex-1 flex items-center justify-center text-sm text-(--color-text-muted)">
              No pull requests found
            </div>
          }
        >
          <div
            ref={scrollRef!}
            class="flex-1 overflow-auto"
            onScroll={handleScroll}
          >
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
                      />
                    </div>
                  );
                }}
              </For>
            </div>

            {/* Load more indicator */}
            <Show when={prListStore.isLoadingMore}>
              <div class="flex justify-center py-4">
                <Spinner size={18} class="text-(--color-text-muted)" />
              </div>
            </Show>
          </div>
        </Show>
      </Show>
    </div>
  );
}
