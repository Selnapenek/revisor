import { createEffect, createSignal, For, onCleanup, Show } from "solid-js";
import { Button } from "~/components/ui/Button";
import { focusHasKey } from "~/api/focus";
import { searchSemanticDiff } from "~/api/semantic";
import { canApplySemanticResult, prepareSemanticSnapshot, semanticLineRange, type SemanticHit, type SemanticResult } from "~/lib/semantic-search";
import type { ProcessedFile } from "~/api/files";
import type { PullRequest } from "~/types/pr";
import type { CommitSelection } from "~/stores/pr-detail.store";

interface SemanticSearchPanelProps {
  owner: string;
  repo: string;
  pr: PullRequest;
  files: ProcessedFile[];
  selection: CommitSelection;
  filesLoading: boolean;
  active: boolean;
  onSelect: (hit: SemanticHit) => void;
  onShowKey: () => void;
  onInvalidate: () => void;
}

export function SemanticSearchPanel(props: SemanticSearchPanelProps) {
  const [query, setQuery] = createSignal("");
  const [hasKey, setHasKey] = createSignal<boolean | null>(null);
  const [busy, setBusy] = createSignal(false);
  const [result, setResult] = createSignal<SemanticResult | null>(null);
  const [error, setError] = createSignal("");
  let requestId = 0;
  let keyReadId = 0;
  let disposed = false;

  createEffect(() => {
    if (!props.active) return;
    const id = ++keyReadId;
    void focusHasKey().then(
      (saved) => { if (!disposed && id === keyReadId) setHasKey(saved); },
      () => { if (!disposed && id === keyReadId) setError("Could not read the TypeSafe key status."); },
    );
  });
  onCleanup(() => { disposed = true; requestId++; keyReadId++; });

  createEffect(() => {
    // Keep the query, but discard results when its context changes.
    const context = `${query()}:${props.owner}/${props.repo}/${props.pr.number}@${props.pr.base_sha}:${props.pr.head_sha}`
      + `:${JSON.stringify(props.selection)}:${props.filesLoading}`
      + `:${props.files.map((file) => `${file.filename}@${file.sha}`).join("|")}`;
    void context;
    requestId++;
    setBusy(false);
    setResult(null);
    setError("");
    props.onInvalidate();
  });

  async function search() {
    const text = query().trim();
    if (!text || busy() || props.filesLoading || props.selection.type !== "all") return;
    if (hasKey() !== true) {
      setError("Save a TypeSafe API key in Files → Review focus → Key.");
      return;
    }
    const snapshot = prepareSemanticSnapshot(props.owner, props.repo, props.pr, props.files);
    const id = ++requestId;
    setBusy(true);
    setResult(null);
    setError("");
    try {
      const next = await searchSemanticDiff(snapshot, text);
      if (!disposed && canApplySemanticResult(
        id, requestId, text, query(), snapshot, props.owner, props.repo,
        props.pr, props.selection, props.filesLoading,
      )) setResult(next);
    } catch (err) {
      if (!disposed && id === requestId) setError(`Semantic search failed: ${err}`);
    } finally {
      if (!disposed && id === requestId) setBusy(false);
    }
  }

  return (
    <section class="h-full flex flex-col text-xs" aria-label="Semantic search">
      <div class="px-3 py-3 border-b border-(--color-border-muted) shrink-0">
        <label for="semantic-query" class="font-medium text-(--color-text-primary)">Semantic diff search</label>
        <p class="mt-1 text-[10px] text-(--color-text-muted)">Sends your query and PR diff text to TypeSafe.</p>
        <div class="mt-2 flex gap-1">
          <input id="semantic-query" type="search" value={query()}
            onInput={(event) => setQuery(event.currentTarget.value)}
            onKeyDown={(event) => { if (event.key === "Enter") { event.preventDefault(); void search(); } }}
            placeholder="Where did retry behavior change?"
            class="flex-1 min-w-0 px-2 py-1 rounded bg-(--color-bg-tertiary) border border-(--color-border-default) text-(--color-text-primary)" />
          <Button size="sm" onClick={search} loading={busy()} disabled={!query().trim() || hasKey() === null || busy() || props.filesLoading || props.selection.type !== "all"}>Search</Button>
        </div>
        <Show when={props.selection.type !== "all"}><p class="mt-2 text-(--color-text-muted)">Switch to All commits to search the full PR.</p></Show>
        <Show when={props.filesLoading}><p class="mt-2 text-(--color-text-muted)">Loading files…</p></Show>
        <Show when={hasKey() === null}><p class="mt-2 text-(--color-text-muted)">Checking TypeSafe key…</p></Show>
        <Show when={hasKey() === false}><button class="mt-2 text-(--color-accent-fg) hover:underline cursor-pointer" onClick={props.onShowKey}>Set TypeSafe key in Review focus</button></Show>
        <Show when={error()}><p class="mt-2 text-(--color-danger-fg)" role="alert">{error()}</p></Show>
      </div>
      <div class="flex-1 min-h-0 overflow-auto px-2 py-2">
        <Show when={result()}>
          {(found) => <>
            <p class="px-1 text-(--color-text-muted)">{found().analyzedCount} files analyzed · {found().skipped.length} not analyzed</p>
            <Show when={found().error}><p class="px-1 mt-1 text-(--color-danger-fg)" role="alert">{found().error}</p></Show>
            <Show when={found().results.length > 0} fallback={<p class="px-1 mt-3 text-(--color-text-secondary)">{found().error || found().skipped.length ? "No matches in completed analysis. Some files were not searched." : "No relevant diff sections found."}</p>}>
              <ol class="mt-2 space-y-1"><For each={found().results}>{(hit) => <li>
                <button class="w-full text-left rounded px-2 py-2 hover:bg-(--color-bg-tertiary) cursor-pointer" onClick={() => props.onSelect(hit)}>
                  <span class="block truncate text-(--color-accent-fg)" title={hit.filename}>{hit.filename}</span>
                  <span class="text-[10px] text-(--color-text-muted)">{hit.score >= 1.5 && hit.confidence >= 0.6 ? "Relevant" : "Possible match"} · {(() => {
                    const hunk = props.files.find((file) => file.filename === hit.filename)?.diff?.hunks[hit.hunkIndex];
                    return hunk
                      ? `old ${semanticLineRange(hunk.old_start, hunk.old_count)}, new ${semanticLineRange(hunk.new_start, hunk.new_count)}`
                      : `old ${hit.oldStart}, new ${hit.newStart}`;
                  })()}</span>
                  <span class="block mt-1 font-mono text-[10px] text-(--color-text-secondary) break-all line-clamp-2">{hit.preview}</span>
                </button>
              </li>}</For></ol>
            </Show>
            <Show when={found().skipped.length > 0}><details class="mt-2 px-1 text-(--color-text-muted)">
              <summary class="cursor-pointer">Files not analyzed</summary>
              <ul class="mt-1 space-y-1"><For each={found().skipped}>{(file) => <li class="break-all">{file.filename}: {file.reason}</li>}</For></ul>
            </details></Show>
          </>}
        </Show>
      </div>
    </section>
  );
}
