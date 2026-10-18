import assert from "node:assert/strict";
import test from "node:test";
import { createPrListCache, prListQueryKey } from "../src/lib/pr-list-cache.ts";
import type { PullRequest } from "../src/types/pr.ts";

const filters = { authors: [], labels: ["bug", "urgent"], assignees: [], review_status: null };

test("restores rows, count, and page for the same query", () => {
  const cache = createPrListCache(2);
  const key = prListQueryKey("o/r", "closed", filters, " fix ");
  cache.set(key, { items: [{ number: 8 } as PullRequest], page: 3, hasNextPage: true, totalCount: 83 });
  const same = prListQueryKey("o/r", "closed", { ...filters, labels: ["urgent", "bug"] }, "fix");
  assert.deepEqual(cache.get(same), { items: [{ number: 8 }], page: 3, hasNextPage: true, totalCount: 83 });
  assert.equal(cache.get(prListQueryKey("o/r", "open", filters, "fix")), undefined);
});

test("evicts the oldest query", () => {
  const cache = createPrListCache(2);
  for (const query of ["one", "two", "three"]) {
    cache.set(prListQueryKey("o/r", "open", filters, query), { items: [], page: 1, hasNextPage: false, totalCount: 0 });
  }
  assert.equal(cache.get(prListQueryKey("o/r", "open", filters, "one")), undefined);
  assert.ok(cache.get(prListQueryKey("o/r", "open", filters, "three")));
});
