# Related Files Implementation Plan

> For agentic workers: use the executing-plans skill to implement this plan task by task. Keep work in the current workspace.

**Goal:** Let a reviewer visit a supporting change and return to the same review position without moving the navigation layout.

**Architecture:** Add an always-present Related button to each diff file header. It opens an anchored overlay using the existing GPUI popover pattern. Reuse the core related-file matcher and the PR model's navigation history.

**Tech stack:** Rust; gpui-kit and gpui-component locked to 0.7.0. No new dependencies.

## Constraints and current state

- Sidebar geometry and file row positions must not depend on selection or result count. Opening and closing Related must not resize either pane or scroll the diff.
- The source context panel has been removed. Do not restore it or add source previews to the overlay.
- Existing uncommitted changes contain `Relation`, `related_files`, `files::counterpart`, and the `t` shortcut. Build on these changes.
- Results cover changed files in the selected commit scope. Do not fetch or index the full repository.
- Use the existing file filter and diff search. The overlay has no separate search input.
- Files with no matches keep an enabled Related button and show an explicit empty state.

## Interaction specification

- Put a fixed-width Related button before Viewed in the diff file header. Use a constant label without a variable count. Keep header height constant and truncate long filenames.
- Use the same control in normal and sticky headers, with distinct element IDs for the two placements.
- Open a floating list about 360 px wide, constrained to the window, with a scrollable body capped at 320 px. The overlay identifies its source file and says “Changed files in the selected commits.”
- Show path and reason on each row. Use factual labels: “Matching test name,” “Matching source name,” “Same filename stem,” and “Name mentioned in changed lines.” Do not claim a verified call or test relationship from name matching.
- Rank test/source name matches first, then matching stems, then mentions. Use path order to break ties. Exclude the current file and unrelated tests.
- Hide same-folder suggestions behind “Show same-folder files” inside the overlay. This action can change overlay content but cannot change the review layout.
- With no strong matches, show “No related changed files found.” If same-folder suggestions exist, keep the explicit action available.
- Escape and clicking outside dismiss the overlay and restore focus. Keyboard users can reach the button and activate result rows. No new global letter shortcut; retain `t` for the best test/source name match.
- A result click closes the overlay and opens that file's diff. Record the review location once. Back returns to the original file, line and scroll offset.
- Bind the open overlay to the file whose button opened it. Close it when that header leaves view or the selected commit scope changes. It must never silently show another file's results.

## Task 1: Refine result quality

**File:** `crates/revisor-core/src/files.rs`.

- [ ] Keep `related_files(files: &[ProcessedFile], current: usize) -> Vec<(usize, Relation)>` as the shared interface. Remove the `OtherTest` fallback; preserve `SameFolder` as a weak relation.
- [ ] Update tests for ranking, reverse source/test lookup, generic stems, whole-word mentions, exclusion of the current file and unrelated tests, empty input, and deterministic order.
- [ ] Run `cargo test -p revisor-core files::tests`. Do not add tests for label text or UI markup.

## Task 2: Add the stable overlay

**Files:** create `crates/revisor-app/src/screens/pr_detail/related.rs`; modify `diff.rs` and `mod.rs` in that directory. Keep `files.rs` free of selection-dependent layout additions.

- [ ] Build the overlay content in `related.rs`, using `PrModel` and a source path. Keep strong and same-folder results separate in this small view.
- [ ] Register the module and add the fixed-size trigger in `DiffView::file_header`. Pass the header's file index explicitly; do not use global selection to identify an unrelated header.
- [ ] Distinguish the sticky and normal header popover IDs. Use existing patterns in `commits.rs` and `pr_list.rs` for trigger, focus and dismissal.
- [ ] Implement empty state, weak suggestions, keyboard access and dismissal from the interaction specification. Compute results on opening or source-data change, not on every diff row render.
- [ ] Close the overlay on commit-scope change and when its source header is no longer active. Preserve focus and all pane sizes on dismissal.

Task 1 and the overlay shell can be developed independently against the existing matcher interface; integrate after the result rules are settled.

## Task 3: Navigation and verification

**Files:** `crates/revisor-app/src/screens/pr_detail/state.rs`, `diff.rs`, and `related.rs`.

- [ ] Use `PrModel::reveal(path, ScrollTarget::File(path))`; the existing `JumpTo` handler records history. Do not push a second history entry from the overlay.
- [ ] Verify Back at a partially visible row. `Spot` currently stores a line and rows above it; preserve `ListOffset::offset_in_item` as well so the return restores the actual scroll offset.
- [ ] Verify navigation to collapsed and filtered-out targets. Existing reveal behavior clears file filters when needed; source context must not be introduced.
- [ ] Run `cargo check -p revisor-app` and `cargo test -p revisor-app` after integration, plus `git diff --check`. Add a behavior test for any changed history state; no UI snapshot or markup tests.
- [ ] Get the Jean Run environment before testing the app. Use the configured native command `cargo run -p revisor-app`; do not start a second process manually.
- [ ] In a real PR, switch among files with zero, one and many matches. Check identical sidebar bounds, header heights and list viewport height. Open/close the overlay with mouse and keyboard; check unchanged diff offset.
- [ ] Test a long path, narrow window, sticky header, commit-scope change, hidden target, collapsed target, repeated navigation and Back from a partially visible row. Confirm one open overlay and no duplicate jumps.
- [ ] If native UI access remains unavailable, report that limit. Do not claim layout stability from compilation alone.

## Sources and API check

- [GPUI Kit Popover documentation](https://gpui-kit.com/docs/components/popover) documents floating content, triggers, controlled opening and dismissal. The online examples have inconsistent callback signatures; use the locked 0.7.0 API and the local working examples as the implementation reference. Do not upgrade dependencies for this feature.
- [Research note](../../review-context-research.md) records the review-tool comparison and the reason for removing duplicate source content.

## Acceptance

A reviewer can open Related, understand why a supporting file is suggested, visit its diff and return to the same line and scroll offset. Selection and result count never change sidebar geometry. The implementation provides no duplicate source panel or second search flow.
