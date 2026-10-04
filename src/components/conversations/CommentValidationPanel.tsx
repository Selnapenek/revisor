import { createEffect, createSignal, onCleanup, Show } from 'solid-js';
import { getPrHeadSourceSnapshot } from '~/api/source-snapshot';
import { resolveReferencesInWorker } from '~/api/comment-references';
import { rankAiFindings } from '~/api/rank-findings';
import { focusHasKey } from '~/api/focus';
import type { ResolvedCommentContext } from '~/lib/comment-references';
import { referenceContextFiles } from '~/lib/comment-reference-context';
import type { CommentThread } from '~/lib/comments';
import type { PullRequest } from '~/types/pr';
import type { ProcessedFile } from '~/api/files';
import type { CommitSelection } from '~/stores/pr-detail.store';
import { Button } from '~/components/ui/Button';
import type { RankingResult } from '~/lib/rank-findings';

interface Props { owner: string; repo: string; pr: PullRequest; thread: CommentThread; selection: CommitSelection; files: ProcessedFile[]; active: boolean; onShowKey: () => void }

export function CommentValidationPanel(props: Props) {
  const [context, setContext] = createSignal<ResolvedCommentContext | null>(null);
  const [hasKey, setHasKey] = createSignal<boolean | null>(null);
  const [result, setResult] = createSignal<RankingResult | null>(null);
  const [busy, setBusy] = createSignal(false);
  const [error, setError] = createSignal('');
  const [status, setStatus] = createSignal('');
  let requestId = 0;
  let disposed = false;
  const identity = () => JSON.stringify([props.owner, props.repo, props.pr.head_sha, props.thread.root.id, props.thread.root.updated_at, props.thread.root.body, props.selection]);
  createEffect(() => { void identity(); requestId++; setContext(null); setResult(null); setError(''); setStatus(''); });
  createEffect(() => { if (props.active) void focusHasKey().then(setHasKey, () => setError('Could not read TypeSafe key status.')); });
  onCleanup(() => { disposed = true; requestId++; });

  function commentDiff(): string {
    const file = props.files.find((item) => item.filename === props.thread.root.path);
    const hunk = file?.diff?.hunks.find((item) => item.lines.some((line) => line.new_lineno === props.thread.line));
    if (!hunk || file?.diff?.is_binary || file?.diff?.is_too_large) throw new Error('The selected comment has no available PR diff hunk. No content was sent.');
    return `${hunk.header}\n${hunk.lines.map((line) => `${line.kind === 'added' ? '+' : line.kind === 'removed' ? '-' : ' '}${line.content}`).join('\n')}`;
  }

  async function prepare() {
    if (busy()) return;
    if (props.selection.type !== 'all' || props.thread.side !== 'RIGHT' || props.thread.root.outdated || props.thread.isResolved || !/\.(?:[cm]?[jt]s|tsx|jsx)$/i.test(props.thread.root.path)) {
      setError('Select an open, current, right-side TS/JS comment in All commits.'); return;
    }
    const id = ++requestId, key = identity();
    setBusy(true); setError(''); setContext(null); setResult(null); setStatus('Loading PR-head source from GitHub…');
    try {
      const snapshot = await getPrHeadSourceSnapshot(props.owner, props.repo, props.pr.head_sha);
      if (disposed || id !== requestId || key !== identity()) return;
      setStatus('Resolving function references…');
      const found = await resolveReferencesInWorker(snapshot.files, props.thread.root.path, props.thread.line);
      // Check the complete request before showing the send action.
      referenceContextFiles(found, props.pr.head_sha, 64 * 1024, commentDiff());
      if (!disposed && id === requestId && key === identity()) setContext(found);
    } catch (cause) { if (!disposed && id === requestId) setError(String(cause)); }
    finally { if (!disposed && id === requestId) { setBusy(false); setStatus(''); } }
  }

  async function send() {
    const found = context();
    if (!found || busy() || hasKey() !== true) return;
    const id = ++requestId, key = identity();
    setBusy(true); setError(''); setResult(null); setStatus('Checking one comment with TypeSafe…');
    try {
      const files = referenceContextFiles(found, props.pr.head_sha, 64 * 1024, commentDiff());
      if (new TextEncoder().encode(JSON.stringify([files, props.thread.root.body, props.thread.replies.map((reply) => reply.body)])).length > 64 * 1024) throw new Error('Complete request context is too large. No content was sent.');
      const response = await rankAiFindings({ owner: props.owner, repo: props.repo, number: props.pr.number, baseSha: props.pr.base_sha, headSha: props.pr.head_sha, contextKind: 'references', files,
        findings: [{ id: props.thread.root.id, path: props.thread.root.path, line: props.thread.line, body: props.thread.root.body, replies: props.thread.replies.map((reply) => reply.body), htmlUrl: props.thread.root.html_url, author: props.thread.root.author_login }] });
      if (!disposed && id === requestId && key === identity()) setResult(response);
    } catch (cause) { if (!disposed && id === requestId) setError(String(cause)); }
    finally { if (!disposed && id === requestId) { setBusy(false); setStatus(''); } }
  }

  return <div class="h-full overflow-auto p-3 space-y-3 text-xs">
    <h3 class="font-semibold">Validate one comment</h3>
    <p class="text-(--color-text-secondary)">{props.thread.root.path}:{props.thread.line}</p>
    <p class="line-clamp-5">{props.thread.root.body}</p>
    <p class="text-(--color-text-muted)">The app loads the exact PR-head TS/JS source. It sends only this comment, its function, and resolved source references to TypeSafe after you select Send. Static references cannot prove runtime behavior.</p>
    <Show when={props.selection.type !== 'all'}><p>Switch to All commits first.</p></Show>
    <Button size="sm" onClick={prepare} loading={busy()} disabled={busy() || props.selection.type !== 'all'}>Find function and usages</Button>
    <Show when={context()}>{(found) => <div class="space-y-2"><p>Function: <strong>{found().symbol}</strong></p><p>{found().usages.length} resolved reference location(s) in {new Set(found().usages.map((usage) => usage.path)).size} file(s).</p>
      <Show when={hasKey() === false}><button class="text-(--color-accent-fg) hover:underline" onClick={props.onShowKey}>Set TypeSafe key in Review focus</button></Show>
      <Button size="sm" onClick={send} loading={busy()} disabled={busy() || hasKey() !== true}>Send to TypeSafe</Button></div>}</Show>
    <Show when={status()}><p role="status">{status()}</p></Show>
    <Show when={error()}><p class="text-(--color-danger-fg)" role="alert">{error()}</p></Show>
    <Show when={result()}>{(value) => <div class="space-y-1"><Show when={value().error}><p role="alert" class="text-(--color-danger-fg)">{value().error}</p></Show>
      <Show when={value().ranked[0]}>{(item) => <><p>Support: {item().support.replace('_', ' ')} ({Math.round(item().confidence * 100)}% confidence)</p><p>Potential impact: {item().impact.toFixed(1)}/3</p></>}</Show>
      <Show when={value().failed[0]}>{(failure) => <p role="alert" class="text-(--color-danger-fg)">{failure().reason}</p>}</Show>
    </div>}</Show>
  </div>;
}
