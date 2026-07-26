import { onMount, onCleanup, Show, For, createSignal, createMemo, createEffect } from "solid-js";
import { useParams, useNavigate } from "@solidjs/router";
import {
  markFileReviewed,
  unmarkFileReviewed,
  getPrFiles,
  getCompareFiles,
} from "~/api";
import { consumePrefetch, fetchPrDetail } from "~/lib/prefetch-cache";
import type { CommitSelection } from "~/stores/pr-detail.store";
import { prDetailStore, uiStore } from "~/stores";
import { searchStore } from "~/stores/search.store";
import { buildThreads } from "~/lib/comments";
import { FileTree } from "~/components/file-tree/FileTree";
import { ConversationList } from "~/components/conversations/ConversationList";
import { DiffViewer } from "~/components/diff/DiffViewer";
import { CodeSearchBar } from "~/components/diff/CodeSearchBar";
import { Button } from "~/components/ui/Button";
import { Spinner } from "~/components/ui/Spinner";
import { Badge } from "~/components/ui/Badge";
import { relativeTime } from "~/lib/time";
import { CommitSelector } from "~/components/commits/CommitSelector";

const SIDEBAR_MIN = 180;
const SIDEBAR_MAX = 600;
const SIDEBAR_DEFAULT = 260;
const SIDEBAR_STORAGE_KEY = "revisor:sidebar-width";

function readPersistedWidth(): number | null {
  try {
    const raw = localStorage.getItem(SIDEBAR_STORAGE_KEY);
    if (raw === null) return null;
    const n = parseInt(raw, 10);
    return Number.isFinite(n) && n >= SIDEBAR_MIN && n <= SIDEBAR_MAX ? n : null;
  } catch {
    return null;
  }
}

function persistWidth(width: number) {
  try {
    localStorage.setItem(SIDEBAR_STORAGE_KEY, String(width));
  } catch { /* ignore */ }
}

export default function PRDetailPage() {
  const params = useParams<{ owner: string; repo: string; number: string }>();
  const navigate = useNavigate();
  let fileFilterRef: HTMLInputElement | undefined;

  const prNumber = () => parseInt(params.number, 10);
  const [activeTab, setActiveTab] = createSignal<"files" | "conversations">("files");
  const [scrollToThreadId, setScrollToThreadId] = createSignal<number | null>(null);
  const [sidebarWidth, setSidebarWidth] = createSignal(readPersistedWidth() ?? SIDEBAR_DEFAULT);
  const [isResizing, setIsResizing] = createSignal(false);
  const [collapsedFiles, setCollapsedFiles] = createSignal<Set<string>>(new Set());
  let scrollContainerRef: HTMLDivElement | undefined;

  const onResizeStart = (e: PointerEvent) => {
    e.preventDefault();
    const target = e.currentTarget as HTMLElement;
    target.setPointerCapture(e.pointerId);
    setIsResizing(true);

    const startX = e.clientX;
    const startWidth = sidebarWidth();

    const onMove = (ev: PointerEvent) => {
      const delta = ev.clientX - startX;
      setSidebarWidth(Math.min(SIDEBAR_MAX, Math.max(SIDEBAR_MIN, startWidth + delta)));
    };

    const onUp = () => {
      target.releasePointerCapture(e.pointerId);
      target.removeEventListener("pointermove", onMove);
      target.removeEventListener("pointerup", onUp);
      setIsResizing(false);
      persistWidth(sidebarWidth());
    };

    target.addEventListener("pointermove", onMove);
    target.addEventListener("pointerup", onUp);
  };

  const conversationCount = createMemo(() =>
    buildThreads(prDetailStore.comments, prDetailStore.threadResolutions).length
    + prDetailStore.issueComments.length
    + prDetailStore.reviews.filter((r) => r.body.trim()).length
  );

  const expandFile = (path: string) => {
    setCollapsedFiles((prev) => {
      if (!prev.has(path)) return prev;
      const next = new Set(prev);
      next.delete(path);
      return next;
    });
  };

  const toggleFileCollapsed = (path: string) => {
    setCollapsedFiles((prev) => {
      const next = new Set(prev);
      if (next.has(path)) next.delete(path);
      else next.add(path);
      return next;
    });
  };

  // Scroll to thread — expand file if collapsed, then scroll
  createEffect(() => {
    const threadId = scrollToThreadId();
    if (threadId === null) return;
    requestAnimationFrame(() => {
      requestAnimationFrame(() => {
        const el = document.querySelector(`[data-comment-thread="${threadId}"]`);
        if (el) {
          el.scrollIntoView({ behavior: "smooth", block: "center" });
          setScrollToThreadId(null);
        }
      });
    });
  });

  // Expand file when search navigates into a collapsed one
  createEffect(() => {
    const match = searchStore.activeMatch;
    if (match) expandFile(match.filePath);
  });

  onMount(async () => {
    prDetailStore.reset();
    searchStore.reset();

    // Load PR, files, comments, and reviewed state in parallel
    prDetailStore.setIsLoadingFiles(true);
    prDetailStore.setIsLoadingComments(true);

    try {
      const { pr, files, comments, issueComments, reviews, reviewed, resolutions, commits } = await (
        consumePrefetch(params.owner, params.repo, prNumber())
        ?? fetchPrDetail(params.owner, params.repo, prNumber())
      );

      prDetailStore.setPr(pr);
      prDetailStore.setFiles(files);
      prDetailStore.setComments(comments);
      prDetailStore.setIssueComments(issueComments);
      prDetailStore.setReviews(reviews);
      prDetailStore.setReviewedFiles(new Set(reviewed));
      prDetailStore.setThreadResolutions(
        new Map(resolutions.map((r) => [r.root_comment_id, r]))
      );
      prDetailStore.setCommits(commits);

      // Auto-select first file
      if (files.length > 0) {
        prDetailStore.setSelectedFilePath(files[0].filename);
      }
    } catch (err) {
      uiStore.showError(`Failed to load PR: ${err}`);
    } finally {
      prDetailStore.setIsLoadingFiles(false);
      prDetailStore.setIsLoadingComments(false);
    }
  });

  // Keyboard shortcuts
  const handleKeyDown = (e: KeyboardEvent) => {
    const mod = e.metaKey || e.ctrlKey;

    // Cmd+F: open code search
    if (mod && e.key === "f") {
      e.preventDefault();
      searchStore.openCodeSearch();
    }

    // Cmd+P: focus file filter
    if (mod && e.key === "p") {
      e.preventDefault();
      fileFilterRef?.focus();
    }

    // Cmd+Shift+C: toggle conversations panel
    if (mod && e.shiftKey && e.key === "c") {
      e.preventDefault();
      setActiveTab((prev) => (prev === "files" ? "conversations" : "files"));
    }

    // Escape: close code search
    if (e.key === "Escape" && searchStore.codeSearchActive) {
      searchStore.closeCodeSearch();
    }

    // Arrow Left/Right: scroll to prev/next file
    if (e.key === "ArrowLeft" || e.key === "ArrowRight") {
      const tag = (e.target as HTMLElement)?.tagName;
      if (tag === "INPUT" || tag === "TEXTAREA" || (e.target as HTMLElement)?.isContentEditable) return;
      e.preventDefault();
      prDetailStore.navigateFile(e.key === "ArrowLeft" ? -1 : 1, searchStore.filteredFiles);
      scrollToSelectedFile();
    }
  };

  onMount(() => document.addEventListener("keydown", handleKeyDown));

  // Sync sidebar highlight with scroll position via IntersectionObserver
  let scrollObserver: IntersectionObserver | undefined;

  createEffect(() => {
    const files = searchStore.filteredFiles;
    if (!scrollContainerRef || files.length === 0) return;

    scrollObserver?.disconnect();

    scrollObserver = new IntersectionObserver(
      (entries) => {
        for (const entry of entries) {
          if (entry.isIntersecting) {
            const path = (entry.target as HTMLElement).dataset.fileSection;
            if (path) prDetailStore.setSelectedFilePath(path);
          }
        }
      },
      {
        root: scrollContainerRef,
        rootMargin: "0px 0px -90% 0px",
        threshold: 0,
      },
    );

    requestAnimationFrame(() => {
      const sections = scrollContainerRef?.querySelectorAll("[data-file-section]");
      sections?.forEach((el) => scrollObserver!.observe(el));
    });
  });

  onCleanup(() => {
    document.removeEventListener("keydown", handleKeyDown);
    scrollObserver?.disconnect();
    prDetailStore.reset();
    searchStore.reset();
  });

  const scrollToSelectedFile = () => {
    const path = prDetailStore.selectedFilePath;
    if (!path || !scrollContainerRef) return;
    const el = scrollContainerRef.querySelector(`[data-file-section="${CSS.escape(path)}"]`);
    el?.scrollIntoView({ behavior: "smooth", block: "start" });
  };

  const handleSelectFile = (path: string) => {
    prDetailStore.setSelectedFilePath(path);
    expandFile(path);
    if (!scrollContainerRef) return;
    const el = scrollContainerRef.querySelector(`[data-file-section="${CSS.escape(path)}"]`);
    el?.scrollIntoView({ behavior: "smooth", block: "start" });
  };

  const handleToggleReviewed = async (path: string) => {
    const wasReviewed = prDetailStore.isFileReviewed(path);
    // Optimistic update
    prDetailStore.toggleFileReviewed(path);

    try {
      if (wasReviewed) {
        await unmarkFileReviewed(params.owner, params.repo, prNumber(), path);
      } else {
        await markFileReviewed(params.owner, params.repo, prNumber(), path);
      }
    } catch {
      // Rollback on failure
      prDetailStore.toggleFileReviewed(path);
    }
  };

  const handleSelectThread = (filePath: string, threadId: number) => {
    prDetailStore.setSelectedFilePath(filePath);
    expandFile(filePath);
    setScrollToThreadId(threadId);
  };

  const handleBack = () => {
    navigate(`/repos/${params.owner}/${params.repo}/pulls`);
  };

  // Cache the full PR files so we can restore them without re-fetching
  let allPrFiles: typeof prDetailStore.files | null = null;

  const handleCommitSelectionChange = async (selection: CommitSelection) => {
    // Cache full PR files before first commit-scoped selection
    if (!allPrFiles) {
      allPrFiles = prDetailStore.files;
    }

    prDetailStore.setCommitSelection(selection);

    try {
      let newFiles;

      if (selection.type === "all") {
        if (allPrFiles) {
          newFiles = allPrFiles;
        } else {
          newFiles = await getPrFiles(params.owner, params.repo, prNumber());
          allPrFiles = newFiles;
        }
      } else {
        const commits = prDetailStore.commits;
        let base: string;
        let head: string;

        if (selection.type === "single") {
          const commit = commits[selection.index];
          head = commit.sha;
          base = commit.parents[0] ?? prDetailStore.pr!.base_sha;
        } else {
          const fromCommit = commits[selection.from];
          const toCommit = commits[selection.to];
          head = toCommit.sha;
          base = fromCommit.parents[0] ?? prDetailStore.pr!.base_sha;
        }

        newFiles = await getCompareFiles(params.owner, params.repo, base, head);
      }

      prDetailStore.setFiles(newFiles);

      // Auto-select first file in new set
      if (newFiles.length > 0) {
        prDetailStore.setSelectedFilePath(newFiles[0].filename);
      } else {
        prDetailStore.setSelectedFilePath(null);
      }
    } catch (err) {
      uiStore.showError(`Failed to load commit files: ${err}`);
    }
  };

  return (
    <div class="h-full flex flex-col">
      {/* Header bar */}
      <div class="flex items-center gap-3 px-4 py-2 border-b border-(--color-border-muted) bg-(--color-bg-secondary) shrink-0">
        <Button variant="ghost" size="sm" onClick={handleBack}>
          &larr; PRs
        </Button>

        <Show when={prDetailStore.pr}>
          {(pr) => (
            <>
              <Badge
                variant={
                  pr().merged_at ? "info"
                  : pr().state === "closed" ? "danger"
                  : pr().draft ? "default"
                  : "success"
                }
              >
                {pr().merged_at ? "Merged" : pr().state === "closed" ? "Closed" : pr().draft ? "Draft" : "Open"}
              </Badge>

              <span class="text-sm font-medium text-(--color-text-primary) truncate">
                {pr().title}
              </span>
              <span class="text-xs text-(--color-text-muted) shrink-0">
                #{pr().number}
              </span>
              <span class="text-xs text-(--color-text-muted) shrink-0">
                {pr().author_login} &middot; {relativeTime(pr().updated_at)}
              </span>
            </>
          )}
        </Show>
      </div>

      {/* Main content: file tree + diff viewer */}
      <Show
        when={!prDetailStore.isLoadingFiles}
        fallback={
          <div class="flex-1 flex items-center justify-center">
            <Spinner size={24} class="text-(--color-text-muted)" />
          </div>
        }
      >
        <div class="flex-1 flex overflow-hidden">
          {/* Left panel: tabs + file tree / conversations */}
          <div style={{ width: `${sidebarWidth()}px` }} class="shrink-0 border-r border-(--color-border-muted) overflow-hidden flex flex-col">
            {/* Commit selector */}
            <Show when={prDetailStore.commits.length > 0}>
              <CommitSelector
                commits={prDetailStore.commits}
                selection={prDetailStore.commitSelection}
                label={prDetailStore.commitSelectionLabel}
                onSelect={handleCommitSelectionChange}
              />
            </Show>

            {/* Tab switcher */}
            <div class="flex border-b border-(--color-border-muted) shrink-0">
              <button
                class={`flex-1 px-3 py-2 text-xs font-medium cursor-pointer transition-colors border-b-2 ${
                  activeTab() === "files"
                    ? "border-(--color-accent-emphasis) text-(--color-text-primary)"
                    : "border-transparent text-(--color-text-muted) hover:text-(--color-text-secondary)"
                }`}
                onClick={() => setActiveTab("files")}
              >
                Files <span class="text-(--color-text-muted)">{prDetailStore.files.length}</span>
              </button>
              <button
                class={`flex-1 px-3 py-2 text-xs font-medium cursor-pointer transition-colors border-b-2 ${
                  activeTab() === "conversations"
                    ? "border-(--color-accent-emphasis) text-(--color-text-primary)"
                    : "border-transparent text-(--color-text-muted) hover:text-(--color-text-secondary)"
                }`}
                onClick={() => setActiveTab("conversations")}
              >
                Conversations <span class="text-(--color-text-muted)">{conversationCount()}</span>
              </button>
            </div>

            {/* Panel content */}
            <div class="flex-1 overflow-hidden">
              <Show when={activeTab() === "files"}>
                <FileTree
                  files={searchStore.filteredFiles}
                  selectedPath={prDetailStore.selectedFilePath}
                  onSelect={handleSelectFile}
                  onToggleReviewed={handleToggleReviewed}
                  fileFilterRef={(el) => { fileFilterRef = el; }}
                />
              </Show>
              <Show when={activeTab() === "conversations"}>
                <ConversationList
                  onSelectThread={handleSelectThread}
                  activeThreadId={scrollToThreadId()}
                />
              </Show>
            </div>
          </div>

          {/* Resize handle */}
          <div
            class="w-[5px] shrink-0 cursor-col-resize relative group"
            onPointerDown={onResizeStart}
          >
            <div
              class={`absolute inset-y-0 left-[1px] w-[3px] rounded-full transition-colors duration-150 ${
                isResizing() ? "bg-(--color-accent-emphasis)" : "bg-transparent group-hover:bg-(--color-accent-emphasis)"
              }`}
            />
          </div>

          {/* Diff viewer panel */}
          <div class="flex-1 flex flex-col overflow-hidden">
            {/* Sticky toolbar */}
            <div class="shrink-0 flex items-center gap-3 px-4 py-1.5 border-b border-(--color-border-muted) bg-(--color-bg-secondary)">
              <span class="text-xs text-(--color-text-muted)">
                {searchStore.filteredFiles.length} files
              </span>

              {/* Collapse / Expand all */}
              <div class="flex items-center gap-1 shrink-0">
                <button
                  class="px-1.5 py-0.5 text-xs text-(--color-text-muted) hover:text-(--color-text-primary) rounded hover:bg-(--color-bg-tertiary) transition-colors cursor-pointer"
                  onClick={() => setCollapsedFiles(new Set(searchStore.filteredFiles.map((f) => f.filename)))}
                  title="Collapse all files"
                >
                  <svg class="w-3.5 h-3.5" viewBox="0 0 16 16" fill="currentColor">
                    <path d="M4.22 3.22a.75.75 0 0 1 1.06 0L8 5.94l2.72-2.72a.75.75 0 1 1 1.06 1.06L8.53 7.53a.75.75 0 0 1-1.06 0L4.22 4.28a.75.75 0 0 1 0-1.06Zm0 8a.75.75 0 0 1 1.06 0L8 13.94l2.72-2.72a.75.75 0 1 1 1.06 1.06l-3.25 3.25a.75.75 0 0 1-1.06 0l-3.25-3.25a.75.75 0 0 1 0-1.06Z" />
                  </svg>
                </button>
                <button
                  class="px-1.5 py-0.5 text-xs text-(--color-text-muted) hover:text-(--color-text-primary) rounded hover:bg-(--color-bg-tertiary) transition-colors cursor-pointer"
                  onClick={() => setCollapsedFiles(new Set())}
                  title="Expand all files"
                >
                  <svg class="w-3.5 h-3.5" viewBox="0 0 16 16" fill="currentColor">
                    <path d="M4.22 7.78a.75.75 0 0 1 0-1.06l3.25-3.25a.75.75 0 0 1 1.06 0l3.25 3.25a.75.75 0 0 1-1.06 1.06L8 5.06 5.28 7.78a.75.75 0 0 1-1.06 0Zm0 4a.75.75 0 0 1 0-1.06l3.25-3.25a.75.75 0 0 1 1.06 0l3.25 3.25a.75.75 0 1 1-1.06 1.06L8 9.06l-2.72 2.72a.75.75 0 0 1-1.06 0Z" />
                  </svg>
                </button>
              </div>

              <div class="flex-1" />

              {/* Diff mode toggle */}
              <div class="flex gap-0.5 bg-(--color-bg-tertiary) rounded p-0.5 shrink-0">
                <button
                  class={`px-2 py-0.5 text-xs rounded cursor-pointer transition-colors ${
                    prDetailStore.diffMode === "split"
                      ? "bg-(--color-bg-secondary) text-(--color-text-primary) shadow-sm"
                      : "text-(--color-text-secondary) hover:text-(--color-text-primary)"
                  }`}
                  onClick={() => prDetailStore.setDiffMode("split")}
                >
                  Split
                </button>
                <button
                  class={`px-2 py-0.5 text-xs rounded cursor-pointer transition-colors ${
                    prDetailStore.diffMode === "unified"
                      ? "bg-(--color-bg-secondary) text-(--color-text-primary) shadow-sm"
                      : "text-(--color-text-secondary) hover:text-(--color-text-primary)"
                  }`}
                  onClick={() => prDetailStore.setDiffMode("unified")}
                >
                  Unified
                </button>
              </div>
            </div>

            {/* Code search bar */}
            <Show when={searchStore.codeSearchActive}>
              <CodeSearchBar
                query={searchStore.codeSearchQuery}
                onQueryChange={(q) => searchStore.setCodeSearchQuery(q)}
                totalMatches={searchStore.totalMatchCount}
                activeIndex={searchStore.activeMatchIndex}
                onNext={() => searchStore.nextMatch()}
                onPrev={() => searchStore.prevMatch()}
                onClose={() => searchStore.closeCodeSearch()}
              />
            </Show>

            {/* Scrollable multi-file container */}
            <div ref={scrollContainerRef} class="flex-1 overflow-auto">
              <Show
                when={searchStore.filteredFiles.length > 0}
                fallback={
                  <div class="h-full flex items-center justify-center text-sm text-(--color-text-muted)">
                    No files to review
                  </div>
                }
              >
                <For each={searchStore.filteredFiles}>
                  {(file) => (
                    <DiffViewer
                      file={file}
                      comments={prDetailStore.getCommentsForFile(file.filename)}
                      repo={{ owner: params.owner, repo: params.repo, number: prNumber() }}
                      isCollapsed={collapsedFiles().has(file.filename)}
                      onToggleCollapsed={() => toggleFileCollapsed(file.filename)}
                      isReviewed={prDetailStore.isFileReviewed(file.filename)}
                      onToggleReviewed={() => handleToggleReviewed(file.filename)}
                    />
                  )}
                </For>
              </Show>
            </div>
          </div>
        </div>
      </Show>
    </div>
  );
}
