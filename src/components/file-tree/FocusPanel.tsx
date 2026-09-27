import { createEffect, createSignal, For, onCleanup, onMount, Show } from "solid-js";
import { Button } from "~/components/ui/Button";
import { findFocusFiles, focusHasKey, focusRemoveKey, focusSaveKey } from "~/api/focus";
import { canAnalyzeFocusFiles, canApplyFocusResponse, isLatestFocusRequest, prepareFocusSnapshot, type FocusResult } from "~/lib/focus-files";
import type { ProcessedFile } from "~/api/files";
import type { PullRequest } from "~/types/pr";
import type { CommitSelection } from "~/stores/pr-detail.store";

interface FocusPanelProps {
  owner: string;
  repo: string;
  pr: PullRequest;
  files: ProcessedFile[];
  selection: CommitSelection;
  filesLoading: boolean;
  onSelectFile: (path: string) => void;
}

export function FocusPanel(props: FocusPanelProps) {
  const [hasKey, setHasKey] = createSignal(false);
  const [showKeyForm, setShowKeyForm] = createSignal(false);
  const [key, setKey] = createSignal("");
  const [busy, setBusy] = createSignal(false);
  const [saving, setSaving] = createSignal(false);
  const [result, setResult] = createSignal<FocusResult | null>(null);
  const [error, setError] = createSignal("");
  let requestId = 0;
  let keyOperationId = 0;
  let disposed = false;

  onMount(async () => {
    const id = keyOperationId;
    try {
      const exists = await focusHasKey();
      if (!disposed && isLatestFocusRequest(id, keyOperationId)) setHasKey(exists);
    } catch {
      if (!disposed && isLatestFocusRequest(id, keyOperationId)) setError("Could not read the TypeSafe key status.");
    }
  });
  onCleanup(() => { disposed = true; requestId++; });

  createEffect(() => {
    // Invalidate results if the PR version, file set, or selected commits change.
    const context = `${props.owner}/${props.repo}/${props.pr.number}@${props.pr.base_sha}:${props.pr.head_sha}`
      + `:${JSON.stringify(props.selection)}`
      + `:${props.files.map((file) => `${file.filename}@${file.sha}`).join("|")}`;
    void context;
    requestId++;
    setResult(null);
    setError("");
  });

  async function saveKey() {
    if (busy()) return;
    keyOperationId++;
    setSaving(true);
    setError("");
    try {
      await focusSaveKey(key());
      requestId++;
      setResult(null);
      setHasKey(true);
      setKey("");
      setShowKeyForm(false);
    } catch (err) {
      setError(`Could not save key: ${err}`);
    } finally {
      setSaving(false);
    }
  }

  async function removeKey() {
    if (busy()) return;
    keyOperationId++;
    setSaving(true);
    setError("");
    try {
      await focusRemoveKey();
      requestId++;
      setResult(null);
      setHasKey(false);
      setKey("");
      setShowKeyForm(false);
    } catch {
      setError("Could not remove the TypeSafe key.");
    } finally {
      setSaving(false);
    }
  }

  async function analyze() {
    if (!hasKey()) {
      setShowKeyForm(true);
      return;
    }
    const snapshot = prepareFocusSnapshot(props.owner, props.repo, props.pr, props.files);
    if (!canAnalyzeFocusFiles(snapshot, props.owner, props.repo, props.pr, props.selection, props.filesLoading)) return;
    const id = ++requestId;
    setBusy(true);
    setResult(null);
    setError("");
    try {
      const next = await findFocusFiles(snapshot);
      if (!disposed && !props.filesLoading && canApplyFocusResponse(
        id, requestId, snapshot, props.owner, props.repo, props.pr, props.selection,
      )) setResult(next);
    } catch (err) {
      if (!disposed && id === requestId) setError(`Could not find focus files: ${err}`);
    } finally {
      if (!disposed) setBusy(false);
    }
  }

  return (
    <section class="px-3 py-2 border-b border-(--color-border-muted) shrink-0 text-xs" aria-label="Review focus">
      <div class="flex items-center justify-between gap-1">
        <span class="font-medium text-(--color-text-primary)">Review focus <span class="text-(--color-text-muted)">Experimental</span></span>
        <button class="text-(--color-accent-fg) hover:underline cursor-pointer" onClick={() => setShowKeyForm(!showKeyForm())} aria-label="TypeSafe key settings">Key</button>
      </div>
      <p class="mt-1 text-[10px] text-(--color-text-muted)">Sends file names and diff text to TypeSafe. Suggestions do not replace a full review.</p>
      <Show when={showKeyForm()}>
        <div class="mt-2 flex flex-col gap-1.5">
          <label for="typesafe-key" class="text-(--color-text-secondary)">TypeSafe API key</label>
          <input id="typesafe-key" type="password" autocomplete="off" value={key()}
            onInput={(event) => setKey(event.currentTarget.value)}
            class="min-w-0 px-2 py-1 rounded bg-(--color-bg-tertiary) border border-(--color-border-default) text-(--color-text-primary)" />
          <div class="flex gap-1 flex-wrap">
            <Button size="sm" onClick={saveKey} disabled={!key().trim() || saving() || busy()} loading={saving()}>Save</Button>
            <Button size="sm" variant="ghost" onClick={() => { setKey(""); setShowKeyForm(false); }}>Cancel</Button>
            <Show when={hasKey()}><Button size="sm" variant="danger" onClick={removeKey} disabled={saving() || busy()}>Remove key</Button></Show>
          </div>
        </div>
      </Show>
      <Show when={props.selection.type === "all"} fallback={<p class="mt-2 text-(--color-text-muted)">Switch to All commits to find focus files.</p>}>
        <Button size="sm" variant="secondary" class="mt-2" onClick={analyze} loading={busy()} disabled={busy() || saving() || props.filesLoading}>Find focus files</Button>
      </Show>
      <Show when={props.filesLoading}><p class="mt-1 text-(--color-text-muted)">Loading files…</p></Show>
      <Show when={error()}><p class="mt-2 text-(--color-danger-fg)" role="alert">{error()}</p></Show>
      <Show when={result()}>
        {(found) => <>
          <p class="mt-2 text-(--color-text-muted)">{found().analyzedCount} analyzed · {found().skipped.length} not analyzed</p>
          <Show when={found().error}><p class="mt-1 text-(--color-danger-fg)" role="alert">{found().error}</p></Show>
          <Show when={found().suggestions.length > 0} fallback={<p class="mt-2 text-(--color-text-secondary)">{found().error ? "No suggestions from completed analyses." : "No strong focus signals found. Review all files as usual."}</p>}>
            <ul class="mt-1 space-y-1">
              <For each={found().suggestions}>{(item) => <li>
                <button class="w-full text-left rounded px-1 py-1 hover:bg-(--color-bg-tertiary) cursor-pointer" onClick={() => props.onSelectFile(item.filename)}>
                  <span class="block truncate text-(--color-accent-fg)" title={item.filename}>{item.filename}</span>
                  <span class="text-[10px] text-(--color-text-muted)">{item.topic}{item.confidence < 0.6 ? " · Uncertain" : ""}</span>
                </button>
              </li>}</For>
            </ul>
          </Show>
          <Show when={found().skipped.length > 0}>
            <details class="mt-2 text-(--color-text-muted)"><summary class="cursor-pointer">Not analyzed</summary>
              <ul class="mt-1 max-h-28 overflow-auto space-y-1"><For each={found().skipped}>{(file) => <li class="break-all">{file.filename}: {file.reason}</li>}</For></ul>
            </details>
          </Show>
        </>}
      </Show>
    </section>
  );
}
