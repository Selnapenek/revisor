import { test } from "node:test";
import assert from "node:assert/strict";
import { prepareSemanticSnapshot, semanticAnchor, canApplySemanticResult } from "../src/lib/semantic-search.ts";
import type { ProcessedFile } from "../src/api/files.ts";

const pr = { number: 4, base_sha: "base", head_sha: "head" };
const file: ProcessedFile = { sha: "blob", filename: "src/auth.ts", status: "modified", additions: 1, deletions: 1,
  diff: { is_binary: false, is_too_large: false, side_by_side: [], word_diffs: [], hunks: [
    { header: "@@ -5,2 +5,2 @@", old_start: 5, old_count: 2, new_start: 5, new_count: 2, lines: [
      { kind: "removed", content: "old", old_lineno: 5, new_lineno: null },
      { kind: "added", content: "new", old_lineno: null, new_lineno: 5 },
    ] },
    { header: "@@ -9 +8,0 @@", old_start: 9, old_count: 1, new_start: 8, new_count: 0, lines: [
      { kind: "removed", content: "removed", old_lineno: 9, new_lineno: null },
    ] },
  ] },
};

test("semantic snapshot sends original hunk line numbers without display rows", () => {
  const snapshot = prepareSemanticSnapshot("org", "repo", pr, [file]);
  assert.equal(snapshot.headSha, "head");
  assert.deepEqual(snapshot.files[0].hunks[0], { header: "@@ -5,2 +5,2 @@", oldStart: 5, newStart: 5,
    lines: [
      { kind: "removed", content: "old", oldLineno: 5, newLineno: null },
      { kind: "added", content: "new", oldLineno: null, newLineno: 5 },
    ],
  });
  assert.equal(snapshot.files[0].hunks.length, 2);
});

test("semantic anchor prefers added line and supports deletion-only section", () => {
  assert.deepEqual(semanticAnchor(file, 0), { side: "new", line: 5 });
  assert.deepEqual(semanticAnchor(file, 1), { side: "old", line: 9 });
  assert.equal(semanticAnchor(file, 3), null);
});

test("stale semantic response is rejected when query or PR scope changes", () => {
  const snapshot = prepareSemanticSnapshot("org", "repo", pr, [file]);
  assert.equal(canApplySemanticResult(1, 1, "retry", "retry", snapshot, "org", "repo", pr, { type: "all" }, false), true);
  assert.equal(canApplySemanticResult(1, 2, "retry", "retry", snapshot, "org", "repo", pr, { type: "all" }, false), false);
  assert.equal(canApplySemanticResult(1, 1, "retry", "auth", snapshot, "org", "repo", pr, { type: "all" }, false), false);
  assert.equal(canApplySemanticResult(1, 1, "retry", "retry", snapshot, "org", "repo", pr, { type: "single", index: 0 }, false), false);
  assert.equal(canApplySemanticResult(1, 1, "retry", "retry", snapshot, "org", "repo", pr, { type: "all" }, true), false);
});

test("semantic result shows original old and new line ranges", async () => {
  const { semanticLineRange } = await import("../src/lib/semantic-search.ts");
  assert.equal(semanticLineRange(5, 2), "5–6");
  assert.equal(semanticLineRange(9, 1), "9");
  assert.equal(semanticLineRange(8, 0), "none");
});
