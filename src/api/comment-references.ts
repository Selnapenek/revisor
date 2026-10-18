import type { ResolvedCommentContext } from '~/lib/comment-references';

export function resolveReferencesInWorker(files: Record<string, string>, path: string, line: number): Promise<ResolvedCommentContext> {
  return new Promise((resolve, reject) => {
    const worker = new Worker(new URL('../lib/comment-references.worker.ts', import.meta.url), { type: 'module' });
    worker.onmessage = (event: MessageEvent<{ result?: ResolvedCommentContext; error?: string }>) => {
      worker.terminate();
      if (event.data.result) resolve(event.data.result); else reject(new Error(event.data.error ?? 'Reference resolution failed'));
    };
    worker.onerror = () => { worker.terminate(); reject(new Error('Reference worker failed')); };
    worker.postMessage({ files, path, line });
  });
}
