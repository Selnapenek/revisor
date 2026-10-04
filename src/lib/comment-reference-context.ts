import type { RankingFile } from './rank-findings.ts';
import type { ResolvedCommentContext } from './comment-references.ts';

export function referenceContextFiles(found: ResolvedCommentContext, headSha: string, maxBytes: number, diff = ''): RankingFile[] {
  const byPath = new Map<string, string[]>();
  byPath.set(found.definition.path, [`Function ${found.symbol} definition (line ${found.definition.line}):\n${found.definition.text}`]);
  for (const usage of found.usages) {
    const blocks = byPath.get(usage.path) ?? [];
    blocks.push(`Reference at line ${usage.line}:\n${usage.text}`);
    byPath.set(usage.path, blocks);
  }
  const files = [...byPath].map(([path, blocks]) => ({ path, status: 'context', content: blocks.join('\n\n'), diff: '', ref: headSha }));
  files.find((file) => file.path === found.definition.path)!.diff = diff;
  if (files.length > 50) throw new Error('Resolved references span more than 50 files. No content was sent.');
  if (new TextEncoder().encode(JSON.stringify(files)).length > maxBytes) throw new Error('Resolved reference context is too large. No content was sent.');
  return files;
}
