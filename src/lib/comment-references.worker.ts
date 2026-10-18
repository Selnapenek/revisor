import { resolveCommentReferences } from './comment-references';
self.onmessage = (event: MessageEvent<{ files: Record<string, string>; path: string; line: number }>) => {
  try { self.postMessage({ result: resolveCommentReferences(event.data.files, event.data.path, event.data.line) }); }
  catch (error) { self.postMessage({ error: String(error) }); }
};
