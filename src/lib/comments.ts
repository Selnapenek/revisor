import type { ReviewComment } from "~/types/pr";

export interface CommentThread {
  root: ReviewComment;
  replies: ReviewComment[];
  line: number;
  side: "LEFT" | "RIGHT";
}

export function buildThreads(comments: ReviewComment[]): CommentThread[] {
  const byId = new Map<number, ReviewComment>();
  for (const c of comments) byId.set(c.id, c);

  // Separate roots from replies
  const roots: ReviewComment[] = [];
  const repliesByRoot = new Map<number, ReviewComment[]>();

  for (const c of comments) {
    if (c.in_reply_to_id === null || !byId.has(c.in_reply_to_id)) {
      // True root, or orphaned reply (parent was deleted) — treat as root
      roots.push(c);
    } else {
      const list = repliesByRoot.get(c.in_reply_to_id) ?? [];
      list.push(c);
      repliesByRoot.set(c.in_reply_to_id, list);
    }
  }

  const threads: CommentThread[] = [];

  for (const root of roots) {
    const line = root.side === "LEFT" ? root.original_line : root.line;
    if (line == null) continue; // file-level comment, skip inline placement

    const replies = (repliesByRoot.get(root.id) ?? []).sort(
      (a, b) => a.created_at.localeCompare(b.created_at)
    );

    threads.push({
      root,
      replies,
      line,
      side: (root.side === "LEFT" ? "LEFT" : "RIGHT") as "LEFT" | "RIGHT",
    });
  }

  // Sort by line number for stable rendering
  threads.sort((a, b) => a.line - b.line);

  return threads;
}

export function threadsByLine(
  threads: CommentThread[]
): Map<string, CommentThread[]> {
  const map = new Map<string, CommentThread[]>();
  for (const thread of threads) {
    const key = `${thread.line}:${thread.side}`;
    const existing = map.get(key) ?? [];
    existing.push(thread);
    map.set(key, existing);
  }
  return map;
}
