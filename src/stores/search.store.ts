import { createSignal, createMemo } from "solid-js";
import { prDetailStore } from "./pr-detail.store";
import { findMatchesInString } from "~/lib/search";
import { createGlobMatcher } from "~/lib/glob";

export interface SearchMatchLocation {
  filePath: string;
  hunkIdx: number;
  lineIdx: number;
  start: number;
  end: number;
  globalIdx: number;
}

const [fileFilterQuery, setFileFilterQuery] = createSignal("");
const [codeSearchQuery, setCodeSearchQuery] = createSignal("");
const [codeSearchActive, setCodeSearchActive] = createSignal(false);
const [activeMatchIndex, setActiveMatchIndex] = createSignal(0);
const [includePattern, setIncludePattern] = createSignal("");
const [excludePattern, setExcludePattern] = createSignal("");
const [advancedFiltersOpen, setAdvancedFiltersOpen] = createSignal(false);

const includeMatcher = createMemo(() => createGlobMatcher(includePattern()));
const excludeMatcher = createMemo(() => createGlobMatcher(excludePattern()));

const hasActiveGlobFilters = createMemo(
  () => includePattern().trim() !== "" || excludePattern().trim() !== "",
);

const filteredFiles = createMemo(() => {
  const q = fileFilterQuery().toLowerCase();
  const files = prDetailStore.files;
  const incMatcher = includeMatcher();
  const excMatcher = excludeMatcher();

  let result = files;

  if (q) {
    result = result.filter((f) => f.filename.toLowerCase().includes(q));
  }
  if (incMatcher) {
    result = result.filter((f) => incMatcher(f.filename));
  }
  if (excMatcher) {
    result = result.filter((f) => !excMatcher(f.filename));
  }

  return result;
});

const codeSearchResults = createMemo((): SearchMatchLocation[] => {
  const query = codeSearchQuery();
  if (!query) return [];

  const files = prDetailStore.files;
  const results: SearchMatchLocation[] = [];
  let globalIdx = 0;

  for (const file of files) {
    if (!file.diff) continue;
    for (let hunkIdx = 0; hunkIdx < file.diff.hunks.length; hunkIdx++) {
      const hunk = file.diff.hunks[hunkIdx];
      for (let lineIdx = 0; lineIdx < hunk.lines.length; lineIdx++) {
        const line = hunk.lines[lineIdx];
        const matches = findMatchesInString(line.content, query);
        for (const m of matches) {
          results.push({
            filePath: file.filename,
            hunkIdx,
            lineIdx,
            start: m.start,
            end: m.end,
            globalIdx: globalIdx++,
          });
        }
      }
    }
  }

  return results;
});

const activeMatch = createMemo((): SearchMatchLocation | null => {
  const results = codeSearchResults();
  if (results.length === 0) return null;
  const idx = activeMatchIndex() % results.length;
  return results[idx < 0 ? idx + results.length : idx] ?? null;
});

/** Map<"filePath", Map<"hunkIdx:lineIdx", SearchMatchLocation[]>> for O(1) lookup */
const matchesByFileAndLine = createMemo(() => {
  const results = codeSearchResults();
  const active = activeMatch();
  const outer = new Map<string, Map<string, SearchMatchLocation[]>>();

  for (const m of results) {
    let inner = outer.get(m.filePath);
    if (!inner) {
      inner = new Map();
      outer.set(m.filePath, inner);
    }
    const key = `${m.hunkIdx}:${m.lineIdx}`;
    let arr = inner.get(key);
    if (!arr) {
      arr = [];
      inner.set(key, arr);
    }
    arr.push(m);
  }

  // Tag active — stored in globalIdx comparison
  void active; // ensure reactivity

  return outer;
});

const matchCountByFile = createMemo(() => {
  const results = codeSearchResults();
  const counts = new Map<string, number>();
  for (const m of results) {
    counts.set(m.filePath, (counts.get(m.filePath) ?? 0) + 1);
  }
  return counts;
});

export const searchStore = {
  get fileFilterQuery() { return fileFilterQuery(); },
  get codeSearchQuery() { return codeSearchQuery(); },
  get codeSearchActive() { return codeSearchActive(); },
  get activeMatchIndex() { return activeMatchIndex(); },
  get filteredFiles() { return filteredFiles(); },
  get codeSearchResults() { return codeSearchResults(); },
  get activeMatch() { return activeMatch(); },
  get matchesByFileAndLine() { return matchesByFileAndLine(); },
  get matchCountByFile() { return matchCountByFile(); },
  get totalMatchCount() { return codeSearchResults().length; },
  get includePattern() { return includePattern(); },
  get excludePattern() { return excludePattern(); },
  get advancedFiltersOpen() { return advancedFiltersOpen(); },
  get hasActiveGlobFilters() { return hasActiveGlobFilters(); },

  setFileFilterQuery,
  setCodeSearchQuery,
  setIncludePattern,
  setExcludePattern,
  setAdvancedFiltersOpen,

  toggleAdvancedFilters() {
    setAdvancedFiltersOpen((prev) => !prev);
  },

  openCodeSearch() {
    setCodeSearchActive(true);
  },

  closeCodeSearch() {
    setCodeSearchQuery("");
    setCodeSearchActive(false);
    setActiveMatchIndex(0);
  },

  nextMatch() {
    const total = codeSearchResults().length;
    if (total === 0) return;
    setActiveMatchIndex((prev) => (prev + 1) % total);
    switchFileIfNeeded();
  },

  prevMatch() {
    const total = codeSearchResults().length;
    if (total === 0) return;
    setActiveMatchIndex((prev) => (prev - 1 + total) % total);
    switchFileIfNeeded();
  },

  goToMatch(index: number) {
    setActiveMatchIndex(index);
    switchFileIfNeeded();
  },

  reset() {
    setFileFilterQuery("");
    setCodeSearchQuery("");
    setCodeSearchActive(false);
    setActiveMatchIndex(0);
    setIncludePattern("");
    setExcludePattern("");
    setAdvancedFiltersOpen(false);
  },
};

function switchFileIfNeeded() {
  const match = activeMatch();
  if (!match) return;
  if (prDetailStore.selectedFilePath !== match.filePath) {
    prDetailStore.setSelectedFilePath(match.filePath);
  }
}
