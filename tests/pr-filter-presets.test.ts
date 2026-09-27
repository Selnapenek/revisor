import { test } from "node:test";
import assert from "node:assert/strict";
import type { PrSearchFilters } from "../src/types/pr.ts";
import { withAuthorPreset, withReviewRequestedPreset } from "../src/lib/pr-filter-presets.ts";

const filters: PrSearchFilters = {
  authors: ["someone"],
  labels: ["bug"],
  assignees: ["teammate"],
  review_status: "approved",
};

test("author preset replaces authors and keeps other filters", () => {
  assert.deepEqual(withAuthorPreset(filters, "me"), {
    ...filters,
    authors: ["me"],
  });
});

test("review preset sets review requested and keeps other filters", () => {
  assert.deepEqual(withReviewRequestedPreset(filters), {
    ...filters,
    review_status: "review-requested:@me",
  });
});
