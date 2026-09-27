import { test } from "node:test";
import assert from "node:assert/strict";
import { prepareFocusSnapshot, isFocusSnapshotCurrent } from "../src/lib/focus-files.ts";
import type { ProcessedFile } from "../src/api/files.ts";

const pr = {
  number: 21,
  base_sha: "base1",
  head_sha: "head1",
};
const files: ProcessedFile[] = [{
  sha: "blob1", filename: "src/login.ts", status: "modified", additions: 1, deletions: 1,
  diff: {
    hunks: [{ header: "@@ -1 +1 @@", old_start: 1, old_count: 1, new_start: 1, new_count: 1,
      lines: [
        { kind: "removed", content: "return false", old_lineno: 1, new_lineno: null },
        { kind: "added", content: "return true", old_lineno: null, new_lineno: 1 },
      ],
    }],
    side_by_side: [], word_diffs: [], is_binary: false, is_too_large: false,
  },
}];

test("focus snapshot sends full diff hunks without derived display data", () => {
  assert.deepEqual(prepareFocusSnapshot("team", "app", pr, files), {
    owner: "team", repo: "app", number: 21, baseSha: "base1", headSha: "head1",
    files: [{ filename: "src/login.ts", status: "modified", isBinary: false, isTooLarge: false,
      hunks: [{ header: "@@ -1 +1 @@", lines: [
        { kind: "removed", content: "return false" },
        { kind: "added", content: "return true" },
      ] }],
    }],
  });
});

test("missing diff remains unavailable, not an empty safe file", () => {
  const snapshot = prepareFocusSnapshot("team", "app", pr, [{ ...files[0], diff: null }]);
  assert.deepEqual(snapshot.files[0], {
    filename: "src/login.ts", status: "modified", isBinary: false, isTooLarge: false, hunks: [],
  });
});

test("focus result is current only for the same full PR version", () => {
  const snapshot = prepareFocusSnapshot("team", "app", pr, files);
  assert.equal(isFocusSnapshotCurrent(snapshot, "team", "app", pr, { type: "all" }), true);
  assert.equal(isFocusSnapshotCurrent(snapshot, "team", "app", pr, { type: "single", index: 0 }), false);
  assert.equal(isFocusSnapshotCurrent(snapshot, "team", "app", { ...pr, head_sha: "head2" }, { type: "all" }), false);
  assert.equal(isFocusSnapshotCurrent(snapshot, "other", "app", pr, { type: "all" }), false);
});

test("late focus response is rejected after another request starts", async () => {
  const { canApplyFocusResponse } = await import("../src/lib/focus-files.ts");
  const snapshot = prepareFocusSnapshot("team", "app", pr, files);
  assert.equal(canApplyFocusResponse(1, 2, snapshot, "team", "app", pr, { type: "all" }), false);
  assert.equal(canApplyFocusResponse(2, 2, snapshot, "team", "app", pr, { type: "all" }), true);
});

test("only the newest async request can update focus state", async () => {
  const { isLatestFocusRequest } = await import("../src/lib/focus-files.ts");
  assert.equal(isLatestFocusRequest(1, 2), false);
  assert.equal(isLatestFocusRequest(2, 2), true);
});

test("focus analysis waits for full-PR files to finish loading", async () => {
  const { canAnalyzeFocusFiles } = await import("../src/lib/focus-files.ts");
  const snapshot = prepareFocusSnapshot("team", "app", pr, files);
  assert.equal(canAnalyzeFocusFiles(snapshot, "team", "app", pr, { type: "all" }, true), false);
  assert.equal(canAnalyzeFocusFiles(snapshot, "team", "app", pr, { type: "all" }, false), true);
  assert.equal(canAnalyzeFocusFiles(snapshot, "team", "app", pr, { type: "single", index: 0 }, false), false);
});
