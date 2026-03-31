import { onMount, onCleanup, Show, createSignal, createMemo, createEffect } from "solid-js";
import { useParams, useNavigate } from "@solidjs/router";
import {
  getPullRequest,
  getPrFiles,
  getPrComments,
  getReviewedFiles,
  markFileReviewed,
  unmarkFileReviewed,
} from "~/api";
import { prDetailStore, uiStore } from "~/stores";
import { searchStore } from "~/stores/search.store";
import { buildThreads } from "~/lib/comments";
import { FileTree } from "~/components/file-tree/FileTree";
import { ConversationList } from "~/components/conversations/ConversationList";
import { DiffViewer } from "~/components/diff/DiffViewer";
import { Button } from "~/components/ui/Button";
import { Spinner } from "~/components/ui/Spinner";
import { Badge } from "~/components/ui/Badge";
import { relativeTime } from "~/lib/time";

export default function PRDetailPage() {
  const params = useParams<{ owner: string; repo: string; number: string }>();
  const navigate = useNavigate();
  let fileFilterRef: HTMLInputElement | undefined;

  const prNumber = () => parseInt(params.number, 10);
  const [activeTab, setActiveTab] = createSignal<"files" | "conversations">("files");
  const [scrollToThreadId, setScrollToThreadId] = createSignal<number | null>(null);

  const conversationCount = createMemo(() => buildThreads(prDetailStore.comments).length);

  // Scroll to thread after file switch renders
  createEffect(() => {
    const threadId = scrollToThreadId();
    if (threadId === null) return;
    if (!prDetailStore.selectedFile) return;
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

  onMount(async () => {
    prDetailStore.reset();
    searchStore.reset();

    // Load PR, files, comments, and reviewed state in parallel
    prDetailStore.setIsLoadingFiles(true);
    prDetailStore.setIsLoadingComments(true);

    try {
      const [pr, files, comments, reviewed] = await Promise.all([
        getPullRequest(params.owner, params.repo, prNumber()),
        getPrFiles(params.owner, params.repo, prNumber()),
        getPrComments(params.owner, params.repo, prNumber()),
        getReviewedFiles(params.owner, params.repo, prNumber()),
      ]);

      prDetailStore.setPr(pr);
      prDetailStore.setFiles(files);
      prDetailStore.setComments(comments);
      prDetailStore.setReviewedFiles(new Set(reviewed));

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
  };

  onMount(() => document.addEventListener("keydown", handleKeyDown));

  onCleanup(() => {
    document.removeEventListener("keydown", handleKeyDown);
    prDetailStore.reset();
    searchStore.reset();
  });

  const handleSelectFile = (path: string) => {
    prDetailStore.setSelectedFilePath(path);
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
    setScrollToThreadId(threadId);
  };

  const handleBack = () => {
    navigate(`/repos/${params.owner}/${params.repo}/pulls`);
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
          <div class="w-[260px] shrink-0 border-r border-(--color-border-muted) overflow-hidden flex flex-col">
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

          {/* Diff viewer panel */}
          <div class="flex-1 overflow-hidden">
            <Show
              when={prDetailStore.selectedFile}
              fallback={
                <div class="h-full flex items-center justify-center text-sm text-(--color-text-muted)">
                  Select a file to review
                </div>
              }
            >
              {(file) => (
                <DiffViewer
                  file={file()}
                  comments={prDetailStore.fileComments}
                  repo={{ owner: params.owner, repo: params.repo, number: prNumber() }}
                />
              )}
            </Show>
          </div>
        </div>
      </Show>
    </div>
  );
}
