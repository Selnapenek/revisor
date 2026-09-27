import { test } from "node:test";
import assert from "node:assert/strict";
import type { PrSearchFilters } from "../src/types/pr.ts";
import { loadRepositoryFilters, saveRepositoryFilters } from "../src/lib/pr-filter-persistence.ts";

function memoryStorage() {
  const values = new Map<string, string>();
  return {
    getItem(key: string) { return values.get(key) ?? null; },
    setItem(key: string, value: string) { values.set(key, value); },
  };
}

const emptyFilters: PrSearchFilters = {
  authors: [],
  labels: [],
  assignees: [],
  review_status: null,
};

test("filters restore independently for each repository", () => {
  const storage = memoryStorage();
  const first: PrSearchFilters = {
    authors: ["me"], labels: ["bug"], assignees: ["alex"], review_status: "approved",
  };
  saveRepositoryFilters(storage, "org/one", first);
  saveRepositoryFilters(storage, "org/two", { ...emptyFilters, review_status: "required" });

  assert.deepEqual(loadRepositoryFilters(storage, "org/one"), first);
  assert.deepEqual(loadRepositoryFilters(storage, "org/two"), {
    ...emptyFilters, review_status: "required",
  });
});

test("missing or malformed saved filters return an empty filter set", () => {
  const storage = memoryStorage();
  storage.setItem("revisor:pr-filters:org%2Fbroken", "not json");
  storage.setItem(
    "revisor:pr-filters:org%2Finvalid",
    JSON.stringify({ authors: [1], labels: [], assignees: [], review_status: null }),
  );

  assert.deepEqual(loadRepositoryFilters(storage, "org/missing"), emptyFilters);
  assert.deepEqual(loadRepositoryFilters(storage, "org/broken"), emptyFilters);
  assert.deepEqual(loadRepositoryFilters(storage, "org/invalid"), emptyFilters);
});

test("storage errors do not stop filters from loading", () => {
  const storage = {
    getItem() { throw new Error("storage disabled"); },
    setItem() { throw new Error("storage disabled"); },
  };

  assert.deepEqual(loadRepositoryFilters(storage, "org/repo"), emptyFilters);
  assert.doesNotThrow(() => saveRepositoryFilters(storage, "org/repo", emptyFilters));
});
