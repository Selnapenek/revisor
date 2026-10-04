import { createEffect, createMemo, createSignal, For, onCleanup, Show } from 'solid-js';
import { getFileContent, type ProcessedFile } from '~/api/files';
import { focusHasKey } from '~/api/focus';
import { rankAiFindings } from '~/api/rank-findings';
import { Button } from '~/components/ui/Button';
import { buildThreads } from '~/lib/comments';
import { canApplyRanking, countExcludedFindings, rankFindings, rankingFilesError, rankingRevision, selectFindings, type RankingFile, type RankingResult } from '~/lib/rank-findings';
import { prDetailStore } from '~/stores';
import type { PullRequest } from '~/types/pr';
import type { CommitSelection } from '~/stores/pr-detail.store';

interface Props {
  owner: string;
  repo: string;
  pr: PullRequest;
  files: ProcessedFile[];
  selection: CommitSelection;
  filesLoading: boolean;
  commentsLoading: boolean;
  active: boolean;
  onSelectThread: (path: string, id: number) => void;
  onShowKey: () => void;
}

function diffText(file: ProcessedFile): string {
  return file.diff!.hunks.map((hunk) => `${hunk.header}\n${hunk.lines.map((line) => `${line.kind === 'added' ? '+' : line.kind === 'removed' ? '-' : ' '}${line.content}`).join('\n')}`).join('\n');
}

export function FindingsPanel(props: Props) {
  const [authors, setAuthors] = createSignal<string[]>([]);
  const [hasKey, setHasKey] = createSignal<boolean | null>(null);
  const [busy, setBusy] = createSignal(false);
  const [result, setResult] = createSignal<RankingResult | null>(null);
  const [error, setError] = createSignal('');
  const [progress, setProgress] = createSignal('');
  let requestId = 0;
  let keyReadId = 0;
  let disposed = false;

  createEffect(() => {
    if (!props.active) return;
    const id = ++keyReadId;
    void focusHasKey().then(
      (saved) => { if (!disposed && id === keyReadId) setHasKey(saved); },
      () => { if (!disposed && id === keyReadId) setError('Could not read the TypeSafe key status.'); },
    );
  });
  onCleanup(() => { disposed = true; requestId++; keyReadId++; });

  const availableAuthors = createMemo(() => [...new Set(buildThreads(prDetailStore.comments, prDetailStore.threadResolutions).map((thread) => thread.root.author_login))].sort());
  const findings = createMemo(() => selectFindings(prDetailStore.comments, prDetailStore.threadResolutions, authors()));
  const excludedCount = createMemo(() => countExcludedFindings(prDetailStore.comments, prDetailStore.threadResolutions, authors()));
  const contextKey = () => JSON.stringify({ owner: props.owner, repo: props.repo, number: props.pr.number,
    base: props.pr.base_sha, head: props.pr.head_sha, selection: props.selection, loading: props.filesLoading, commentsLoading: props.commentsLoading,
    files: props.files.map((file) => [file.filename, file.sha, file.status, file.diff?.is_binary, file.diff?.is_too_large]),
    authors: authors(), comments: prDetailStore.comments.map((comment) => [comment.id, comment.body, comment.updated_at, comment.outdated]),
    resolutions: [...prDetailStore.threadResolutions.entries()],
  });
  createEffect(() => { void contextKey(); requestId++; setResult(null); setError(''); });

  function toggleAuthor(author: string) {
    setAuthors((current) => current.includes(author) ? current.filter((item) => item !== author) : [...current, author]);
  }

  async function run() {
    if (busy() || props.filesLoading || props.commentsLoading || props.selection.type !== 'all' || hasKey() !== true) return;
    const selected = findings();
    if (!selected.length) return;
    if (selected.length > 50) {
      setError('Ranking supports at most 50 findings. No content was sent.');
      return;
    }
    const fileError = rankingFilesError(props.files);
    if (fileError) { setError(fileError); return; }
    const key = contextKey();
    const id = ++requestId;
    setBusy(true); setResult(null); setError(''); setProgress('Loading complete changed files…');
    try {
      const files: RankingFile[] = new Array(props.files.length);
      let next = 0;
      await Promise.all(Array.from({ length: Math.min(3, props.files.length) }, async () => {
        while (next < props.files.length) {
          const index = next++;
          const file = props.files[index];
          const ref = rankingRevision(file, props.pr);
          const lines = await getFileContent(props.owner, props.repo, file.filename, ref);
          files[index] = { path: file.filename, status: file.status, content: lines.join('\n'), diff: diffText(file), ref };
        }
      }));
      if (!canApplyRanking(id, requestId, key, contextKey()) || disposed) return;
      setProgress(`Ranking ${selected.length} findings…`);
      const response = await rankAiFindings({ owner: props.owner, repo: props.repo, number: props.pr.number,
        baseSha: props.pr.base_sha, headSha: props.pr.head_sha, files, findings: selected });
      if (canApplyRanking(id, requestId, key, contextKey()) && !disposed) setResult({ ...response, ranked: rankFindings(response.ranked) });
    } catch (cause) {
      if (id === requestId && !disposed) setError(`Could not rank findings: ${cause}`);
    } finally {
      if (!disposed) { setBusy(false); setProgress(''); }
    }
  }

  return <div class="h-full flex flex-col text-xs">
    <div class="p-3 border-b border-(--color-border-muted) space-y-2">
      <p class="text-(--color-text-secondary)">Select AI reviewer accounts. Ranking sends their comment text and all changed-file contents to TypeSafe. It does not prove a finding is correct.</p>
      <Show when={availableAuthors().length} fallback={<p>No inline comment authors are available.</p>}>
        <div class="max-h-24 overflow-auto space-y-1"><For each={availableAuthors()}>{(author) =>
          <label class="flex gap-2 items-center"><input type="checkbox" checked={authors().includes(author)} onChange={() => toggleAuthor(author)} />{author}</label>
        }</For></div>
      </Show>
      <p class="text-(--color-text-muted)">{findings().length} open findings · {excludedCount()} excluded</p>
      <Show when={hasKey() === null}><p class="text-(--color-text-muted)">Checking TypeSafe key…</p></Show>
      <Show when={props.selection.type !== 'all'}><p>Switch to All commits to rank findings.</p></Show>
      <Show when={hasKey() === false}><button class="text-(--color-accent-fg) hover:underline" onClick={props.onShowKey}>Set TypeSafe key in Review focus</button></Show>
      <Button size="sm" onClick={run} loading={busy()} disabled={busy() || hasKey() !== true || !findings().length || props.filesLoading || props.commentsLoading || props.selection.type !== 'all'}>Rank findings</Button>
      <Show when={progress()}><p role="status">{progress()}</p></Show>
      <Show when={error()}><p class="text-(--color-danger-fg)" role="alert">{error()}</p></Show>
    </div>
    <div class="flex-1 min-h-0 overflow-auto">
      <Show when={result()}>{(value) => <>
        <Show when={value().error}><p class="p-2 text-(--color-danger-fg)" role="alert">{value().error}</p></Show>
        <For each={value().ranked}>{(item, index) => {
          const finding = () => findings().find((entry) => entry.id === item.id);
          return <Show when={finding()}>{(entry) => <div class="p-3 border-b border-(--color-border-muted)">
            <button class="w-full text-left hover:text-(--color-accent-fg)" onClick={() => props.onSelectThread(entry().path, entry().id)}>
              <span class="font-semibold">{index() + 1}. {entry().path}:{entry().line}</span>
              <span class="block text-(--color-text-muted)">By {entry().author}</span>
              <span class="block text-(--color-text-muted)">Impact {item.impact.toFixed(1)}/3 · {item.effectiveSupport?.replace('_', ' ')} · support confidence {Math.round(item.confidence * 100)}%</span>
              <Show when={item.confidence < 0.6}><span class="block text-(--color-danger-fg)">Low confidence — review manually.</span></Show>
              <span class="block mt-1 line-clamp-3">{entry().body}</span>
            </button>
            <a href={entry().htmlUrl} target="_blank" rel="noopener noreferrer" class="text-(--color-accent-fg) hover:underline">GitHub comment</a>
          </div>}</Show>;
        }}</For>
        <Show when={value().failed.length}><details class="p-2"><summary>{value().failed.length} failed or unprocessed</summary><For each={value().failed}>{(failure) => <p>#{failure.id}: {failure.reason}</p>}</For></details></Show>
      </>}</Show>
    </div>
  </div>;
}
