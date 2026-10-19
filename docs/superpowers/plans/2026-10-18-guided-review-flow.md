# Guided Review Flow Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking. Each task is sized to run in its own session. Read **Global Constraints** and **Task order** first.

**Goal:** Make the PR detail screen tell the reviewer what to do next: understand the PR (description + AI recap), triage AI findings, read the code in a useful order, then submit.

**Architecture:** Reuse what exists. The sidebar AI tab becomes a default **Overview** tab that also holds the PR description, so the right description pane goes away. The header gets one primary "next step" button driven by a small pure function. Findings become one severity-sorted list whose triage is saved in the existing saved `Review` JSON. The existing AI call returns one more field, a reading order, which sorts the existing file list and diff.

**Tech Stack:** Rust; GPUI through `gpui-kit` / `gpui-component` 0.7.0; SQLite settings table through `revisor_core::cache::queries`.

**Spec:** `docs/faster-review-research.md` (Recommendations #1–#9, Proposed flow, Open questions). Line numbers below refer to commit `ad489d0`.

## Global Constraints

- No new dependencies. `gpui-kit` / `gpui-component` stay at 0.7.0.
- AI stays one click, with no setup steps (user rule). AI review never starts on its own when a PR opens: the user must know what code leaves the device first (roadmap R3).
- Saved reviews keep the key `ai-review-v5` (`ai_review::review_key`). Every new `Review` field uses `#[serde(default)]`, so older saved reviews still load.
- Do not insert or remove blocks above a scrolling list when the selection changes (`.ai/lessons.md`).
- Bare-letter key bindings use `NOT_TYPING` (`mod.rs:57`).
- Work that must outlive a screen stays in `ai_runs` (global entity). Do not move AI runs into panels.
- UI copy: tab **Overview**; section **Findings**; disclosure **Details**; files are **Viewed** (never "proofed").
- Match the surrounding style: short `///` doc comments, `cx.listener` closures, `theme::eyebrow` / `theme::stamp` for labels.
- Commit after each task with a conventional message, for example `feat(pr-detail): open on an Overview tab`. No AI attribution lines (user rule).
- Verify each task with `cargo check -p revisor-app`, plus the named `cargo test -p revisor-core <test>` tests. Then check the running app: call Jean MCP `get_run_environments` first. If Jean has no Run command, report that and give manual steps. Do not invent a command.
- After each task, add one line to the progress log in `docs/roadmap.md` (the table near line 219): date, task, result, next step.

## Task order

```
T1 Overview tab ──► T2 Overview layout ──► T3 Findings triage ──► T4 Next-step button
                                                   │
                                                   └──► T6 AI reading order ──► T9 (optional) inline AI markers
T5 Collapse generated files   (independent)
T7 Remove Related section     (independent; run before T6, both edit files.rs)
T8 Tidy-ups                   (after T1; both edit mod.rs sidebar())
```

T5 and T7 can run in parallel with T1. T9 is optional: run it only if trial users ask for it (spec, Open question 2).

## Review Focus

1. **Old saved reviews** (JSON without `done` or `order`) must still load and show findings. Tests: T3 `old_review_without_new_fields_loads` (T6 extends it).
2. **AI order with bad names**: the AI lists a file that is not in the PR, lists a file twice, or leaves files out. Every changed file must still show exactly once. Tests: T6 `reading_order_*`.
3. **Filter plus AI order**: files hidden by the file filter must stay hidden when the AI order is applied. Test: T6 `reading_order_keeps_only_visible_files`.
4. **New head commit after a review**: dismissals, order and the findings count of the old review must not apply. The next step goes back to "AI review". Tests: T4 `outdated_review_asks_for_ai_review`; T6 manual check.
5. **Dismiss while a run for the same PR is in progress**: saving the old review must not overwrite the new saved one. T3 skips the save while `busy`. T3 has a manual check.

---

### Task 1: Overview tab replaces the AI tab and the description pane

**Files:**
- Modify: `crates/revisor-app/src/screens/pr_detail/mod.rs` (enum `SidebarTab` :138-144, `new` :221-225, `header` :332-344, `ai_review` :348-355, `description` :362-375, `sidebar` :410-444, `render` :452-525, `DESCRIPTION_WIDTH` :60)
- Modify: `crates/revisor-app/src/screens/pr_detail/ai.rs` (struct `AiPanel` :39-53, `new` :60-99, `render` :472-531)

**Interfaces:**
- Produces: `SidebarTab::Overview` (replaces `SidebarTab::Ai`). `AiPanel` shows the PR description at its top. `AiPanel` observes the `PrModel` (T2, T3 and T6 rely on this observer).

- [ ] **Step 1: Rename and reorder the tabs.** In `mod.rs`, set `enum SidebarTab { Overview, Files, Conversations, Semantic }`. Labels: `"Overview"`, `"Files {n}"`, `"Conversations {n}"`, `"Semantic"`. Keep `self.tab as usize` as the tab index and keep the `ix >= 3` Semantic guard, so the enum order must match the label order. In `on_click`, use the matching array `[Overview, Files, Conversations, Semantic]`. `ToggleConversations` and `FocusFileFilter` keep their Files/Conversations behavior.
- [ ] **Step 2: Open on Overview.** `new`: `tab: SidebarTab::Overview`. `ai_review()`: `self.tab = SidebarTab::Overview`.
- [ ] **Step 3: Remove the right pane.** Delete `description_open`, `DESCRIPTION_WIDTH`, `fn description`, the `description-toggle` header button and the third `resizable_panel`. Remove the `TextView` and `video` imports from `mod.rs` if nothing else uses them.
- [ ] **Step 4: Show the description in `AiPanel`.** Add the field `description_open: bool` (default `true`) and `fn description(&self, cx: &Context<Self>) -> impl IntoElement`. The header row is `theme::eyebrow("Description")` with a chevron toggle (`IconName::ChevronDown` / `ChevronRight`, as in `files.rs:368`). The body is `TextView::markdown("pr-description", body).plugin(video::Videos)`. If the body is empty, show the muted line `No description.` Render it first in the scroll body `ai-body`, before the run controls. Move the run controls from their fixed `actions` block into the scroll body under the description, so the tab reads top to bottom.
- [ ] **Step 5: Re-render on model changes, but not on scroll.** In `AiPanel::new`, add `cx.observe(&model, …)`. It calls `cx.notify()` only when the key `(head_sha, body, files_loading)` differs from a stored `last_model: (Option<String>, Option<String>, bool)`. Reason: the model notifies on every scroll frame (`set_selected_from_scroll`), and the tab is a cached view (`mod.rs:433`).
- [ ] **Step 6: Verify.** Run `cargo check -p revisor-app`. Expected: no errors and no new warnings. In the app, open a PR. The Overview tab is selected, the description is at its top and collapses, there is no right pane, and the diff uses the freed width. The **AI review** header button still switches to Overview and runs.
- [ ] **Step 7: Commit** `feat(pr-detail): open PRs on an Overview tab with the description`.

### Task 2: Overview layout — recap first, details collapsed, pickers in Settings

**Files:**
- Modify: `crates/revisor-app/src/screens/pr_detail/ai.rs` (`picker` :229-259, `choices` :262-304, `pick_*` :144-165, `results` :365-469, `render` :472-531)
- Modify: `crates/revisor-app/src/screens/settings.rs` (`ai_page` :209-283, `set_ai` :188-207, agent loading)

**Interfaces:**
- Consumes: T1's description block and model observer.
- Produces: in `AiPanel`, the field `details_open: bool` and the fixed section order below. Settings writes the `ai-backend` setting (the same key that `ai_review::saved_choice` reads, `ai_review.rs:361`).

- [ ] **Step 1: Fix the section order in the scroll body.**
  1. Description (T1).
  2. Run row. With no review: the existing privacy text, then the line `Runs {backend} · {model} · {effort} effort`, a link button `Change in Settings` that dispatches `app::OpenSettings`, and the primary **Run AI review**. With a review: one muted line `AI recap · {backend} · {short sha} · {s}s{cost}` and a ghost xsmall **Run again** / **Review new changes**. **Cancel** shows while busy. The outdated warning (`ai.rs:403-407`) stays directly above the summary.
  3. Summary markdown.
  4. One stamp row: `{door} door` stamp, muted `Blast radius: {x}`, spec stamp (same labels and colors as `ai.rs:370-376`). Under it, the `danger.note` line.
  5. Findings (keep the existing `groups` here; T3 replaces them).
  6. **Details** disclosure, closed by default (`details_open: false`). Inside: Evidence, Spec note, Standards check (all three sub-blocks), and "Not reviewed n".
- [ ] **Step 2: Remove the pickers from the panel.** Delete `picker`, `choices`, `pick_backend`, `pick_model`, `pick_effort` and `save`. Keep `Agent`, `load_agents`, `reload_agents` and `choice()`, because a run still needs them. Delete `Agent::current` only if it becomes unused.
- [ ] **Step 3: Add the CLI choice to Settings.** On each CLI page from `ai_page`, add a first group `"AI review"` with the item `"Review with"`: a `SettingField::dropdown` with options `Claude Code` / `Codex`, limited to installed CLIs. It reads and writes the `ai-backend` setting (value `Backend::executable()`). The write uses `backend::block_on`, as in `set_ai`, so that Back shows the new value. Both pages show the same value. The default is the first installed CLI, as in `saved_choice`. `AiPanel::reload_agents` already rereads the setting when the PR screen returns from Settings (`mod.rs:179`).
- [ ] **Step 4: Verify.** Run `cargo check -p revisor-app`. In the app, check the order above. Details opens and closes. Change the CLI in Settings, go back to the PR, and confirm that the run line shows the new CLI. Start a background review from the PR list; it uses the same CLI.
- [ ] **Step 5: Commit** `feat(pr-detail): put the AI recap first and move AI pickers to Settings`.

### Task 3: One findings list with saved triage

**Files:**
- Modify: `crates/revisor-core/src/ai_review.rs` (`Review` :119-145, `run` :392-413, tests :416+)
- Modify: `crates/revisor-app/src/screens/pr_detail/ai.rs` (`done` :48-49, :83, :94; `draft` :215-224; `comment_row` :306-359; `groups` in `results` :387-401)

**Interfaces:**
- Produces (core): `pub fn severity_rank(c: &Comment) -> (usize, usize)` returns (severity index in `["high","medium","low"]`, axis index in `AXES`); unknown values sort last. `Review.done: Vec<usize>`, documented as "Indexes into `comments` that the user dismissed or turned into a draft."
- Produces (app): `AiPanel::open_findings(&self, cx: &App) -> Option<usize>`. It returns `None` when there is no review or when the review is not current (`!is_current`); otherwise it returns the number of comments whose index is not in `done`. T4 uses it.

- [ ] **Step 1: Write the failing core tests** in `ai_review.rs` tests. Add the helper `fn c(axis: &str, severity: &str) -> Comment` (file `a.rs`, line 1, side `new`, empty rule and body).

```rust
#[test]
fn comments_sort_by_severity_then_axis() {
    let mut v = vec![c("spec", "low"), c("standards", "high"), c("bug", "medium"), c("bug", "high"), c("x", "y")];
    v.sort_by_key(severity_rank);
    let got: Vec<_> = v.iter().map(|c| (c.axis.as_str(), c.severity.as_str())).collect();
    assert_eq!(got, [("bug", "high"), ("standards", "high"), ("bug", "medium"), ("spec", "low"), ("x", "y")]);
}

#[test]
fn old_review_without_new_fields_loads() {
    let json = r#"{"summary":"s","evidence":"e","danger":{"door":"two-way","blast_radius":"ui","note":"n"},
        "spec":{"status":"matches","note":""},"standards_check":{"applicable":"","passing":"","unchecked":""},
        "comments":[],"head_sha":"abc"}"#;
    let review: Review = serde_json::from_str(json).unwrap();
    assert!(review.done.is_empty());
}
```

- [ ] **Step 2: Run** `cargo test -p revisor-core -- comments_sort_by_severity_then_axis old_review_without_new_fields_loads`. Expected: compile FAIL (`severity_rank` and `done` are not defined).
- [ ] **Step 3: Implement.** Add `severity_rank` and `#[serde(default)] pub done: Vec<usize>`. In `run`, replace the inline `rank` closure with `review.comments.sort_by_key(severity_rank)`.
- [ ] **Step 4: Run the same tests.** Expected: PASS.
- [ ] **Step 5: Use `review.done` in the panel.** Remove `done: HashSet<usize>` and `this.done.clear()` (the new review comes with an empty `done`). Add `fn mark_done(&mut self, index: usize, cx: &mut Context<Self>)`: add the index if it is missing, then save the review with `ai_review::save` through `backend::call`, detached. **Skip the save while `self.busy(cx)`**: the running job saves a new review under the same key, and an old JSON must not overwrite it (Review Focus 5). Dismiss and `draft` call `mark_done`.
- [ ] **Step 6: Render one list.** Replace the three `AXES` groups with one section: eyebrow `Findings · {open} of {total} left`. Rows are the open comments, sorted with `severity_rank` at render time, so older saved reviews also show sorted. Each row shows the severity stamp, then an axis stamp (the title from `AXES`), then the location link. With zero comments: `No findings. The AI can miss problems.` If `done` is not empty: the muted button `{n} done · Show`, which toggles `show_done: bool`. Done rows show muted, with a ghost **Restore** button that removes the index from `done` and saves it (same busy guard). Add `open_findings`.
- [ ] **Step 7: Verify.** Run `cargo check -p revisor-app`. In the app, dismiss one finding and add another as a comment. The count drops by two. Restart the app; the count and the done list are the same. Restore one finding. Start **Run again** and dismiss a finding while it runs; when the run finishes, the new review shows with all findings open.
- [ ] **Step 8: Commit** `feat(ai-review): sort findings by severity and save triage`.

### Task 4: One primary "next step" button in the header

**Files:**
- Modify: `crates/revisor-app/src/screens/pr_detail/mod.rs` (`header` :244-345, new tests module at the end of the file)

**Interfaces:**
- Consumes: `AiPanel::busy(&self, cx: &App) -> bool` (`ai.rs:176`); `AiPanel::open_findings` (T3); `PrModel::navigate_unreviewed(true, cx)` (`state.rs:408`); `PrModel::pending_count()`; `PrModel::reviewed`.
- Produces: `#[derive(Debug, PartialEq)] enum Step { RunAi, AiRunning, Findings(usize), NextFile, Finish }` and `fn next_step(ai_busy: bool, open_findings: Option<usize>, unviewed: usize) -> Step`.

- [ ] **Step 1: Write the failing tests** in `mod.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn busy_ai_wins() { assert_eq!(next_step(true, Some(3), 5), Step::AiRunning); }
    #[test]
    fn outdated_review_asks_for_ai_review() { assert_eq!(next_step(false, None, 5), Step::RunAi); }
    #[test]
    fn open_findings_come_before_files() { assert_eq!(next_step(false, Some(2), 5), Step::Findings(2)); }
    #[test]
    fn then_unviewed_files() { assert_eq!(next_step(false, Some(0), 5), Step::NextFile); }
    #[test]
    fn then_finish() { assert_eq!(next_step(false, Some(0), 0), Step::Finish); }
}
```

- [ ] **Step 2: Run** `cargo test -p revisor-app -- busy_ai_wins outdated_review open_findings then_`. Expected: compile FAIL.
- [ ] **Step 3: Implement `next_step`** with the rules in the order the tests show: busy, then no current review, then open findings, then unviewed files, then finish.
- [ ] **Step 4: Run the tests.** Expected: PASS.
- [ ] **Step 5: Use it in the header.** `unviewed` = the number of `model.files` whose name is not in `model.reviewed`. Replace the `ai-review` button with one `step` button, which is **primary** in all cases except `Finish`:
  - `RunAi`: label `AI review`, tooltip as today, click `self.ai_review(cx)`.
  - `AiRunning`: label `AI reviewing…`, `.loading(true)`, click switches to Overview.
  - `Findings(n)`: label `Findings: {n} left`, click sets `tab = Overview` and `sidebar_open = true`.
  - `NextFile`: label `Next unviewed file`, `tooltip_with_action("Next unviewed file", &NextUnreviewed, None)`, click `navigate_unreviewed(true)`.
  - `Finish`: do not render the step button.
  The `review` button is primary only when the step is `Finish`; otherwise it is ghost. Its label stays `Finish review ({n})` when a pending review exists and `Review` when not. Make the sidebar toggle icon-only: `IconName::Menu`, tooltip `Show or hide sidebar`, `.selected(self.sidebar_open)`.
- [ ] **Step 6: Verify.** Run `cargo check -p revisor-app`. In the app, walk one PR through each label: no review, running, findings left, next unviewed file (it jumps to the file), and the Finish dialog. Push a new commit to the PR (or use a PR with an outdated saved review); the button shows `AI review`.
- [ ] **Step 7: Commit** `feat(pr-detail): show one primary next-step button`.

### Task 5: Collapse lock and generated files by default

**Files:**
- Modify: `crates/revisor-core/src/ai_review.rs` (`skip_reason` :267-278, tests)
- Modify: `crates/revisor-app/src/screens/pr_detail/state.rs` (:237)

**Interfaces:**
- Produces: `pub fn is_generated(filename: &str) -> bool` in `ai_review.rs`. The `GENERATED` list moves to module level, with its values unchanged: `[".lock", "-lock.json", "-lock.yaml", ".min.js", ".snap", ".svg", "go.sum"]`.

- [ ] **Step 1: Write the failing test:**

```rust
#[test]
fn generated_files_match_lock_and_build_outputs() {
    for name in ["Cargo.lock", "web/package-lock.json", "pnpm-lock.yaml", "dist/app.min.js", "go.sum", "icons/a.svg", "x.snap"] {
        assert!(is_generated(name), "{name}");
    }
    for name in ["src/lock.rs", "lockfile.md", "src/main.rs"] {
        assert!(!is_generated(name), "{name}");
    }
}
```

- [ ] **Step 2: Run** `cargo test -p revisor-core generated_files_match_lock_and_build_outputs`. Expected: compile FAIL.
- [ ] **Step 3: Implement `is_generated`.** Make `skip_reason` call it.
- [ ] **Step 4: Run the test.** Expected: PASS. Also run `cargo test -p revisor-core prompt_numbers_lines_and_lists_skipped_files`. Expected: PASS.
- [ ] **Step 5: Collapse on load.** At `state.rs:237`, the fallback becomes `reviewed` plus every `files` entry where `ai_review::is_generated(&f.filename)`. Do not change the branch that keeps the user's collapse choices when the head is the same.
- [ ] **Step 6: Verify.** Run `cargo check -p revisor-app`. Open a PR that changes `Cargo.lock`: the file shows collapsed, and a click expands it.
- [ ] **Step 7: Commit** `feat(pr-detail): collapse lock and generated files by default`.

### Task 6: AI reading order sorts the file list and the diff

**Files:**
- Modify: `crates/revisor-core/src/ai_review.rs` (`SYSTEM` :30-49, `Review`, `schema` :149-171, tests)
- Modify: `crates/revisor-app/src/screens/pr_detail/state.rs` (`PrModel` fields :73-120, `set_files` :250-258, `refilter` :260-265)
- Modify: `crates/revisor-app/src/screens/pr_detail/ai.rs` (load callback :68-73, `Finished` handler :80-85, the T1 model observer)
- Modify: `crates/revisor-app/src/screens/pr_detail/files.rs` (view buttons near :462, flat rows `rebuild_rows` :187-200, row render near :259, Review focus :530)

**Interfaces:**
- Produces (core): `#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)] pub struct FileOrder { pub file: String, pub reason: String }`. `Review.order: Vec<FileOrder>` with `#[serde(default)]`. `pub fn reading_order(files: &[ProcessedFile], visible: &[usize], order: &[FileOrder]) -> Vec<usize>`.
- Produces (app): `PrModel.reading_order: Vec<FileOrder>`, `PrModel.ai_order: bool` (default `true`), `PrModel::set_reading_order(&mut self, order: Vec<FileOrder>, cx)`, `PrModel::set_ai_order(&mut self, on: bool, cx)`, `PrModel::order_reason(&self, path: &str) -> Option<&str>`.

- [ ] **Step 1: Write the failing core tests** (use the existing `file()` helper in the tests module):

```rust
fn order(names: &[&str]) -> Vec<FileOrder> {
    names.iter().map(|n| FileOrder { file: n.to_string(), reason: String::new() }).collect()
}

#[test]
fn reading_order_puts_listed_files_first_and_keeps_the_rest() {
    let files = [file("a.rs", "", "x"), file("b.rs", "", "x"), file("c.rs", "", "x")];
    assert_eq!(reading_order(&files, &[0, 1, 2], &order(&["c.rs", "a.rs"])), [2, 0, 1]);
}

#[test]
fn reading_order_ignores_unknown_and_duplicate_files() {
    let files = [file("a.rs", "", "x"), file("b.rs", "", "x")];
    assert_eq!(reading_order(&files, &[0, 1], &order(&["zzz.rs", "b.rs", "b.rs"])), [1, 0]);
}

#[test]
fn reading_order_keeps_only_visible_files() {
    let files = [file("a.rs", "", "x"), file("b.rs", "", "x"), file("c.rs", "", "x")];
    assert_eq!(reading_order(&files, &[0, 2], &order(&["b.rs", "c.rs"])), [2, 0]);
}
```

Also add `assert!(review.order.is_empty());` to the T3 test `old_review_without_new_fields_loads`.

- [ ] **Step 2: Run** `cargo test -p revisor-core -- reading_order old_review_without_new_fields_loads`. Expected: compile FAIL.
- [ ] **Step 3: Implement core.** Add `FileOrder`, the `order` field and `reading_order`. The result is the listed files (only indexes in `visible`, first occurrence only) in AI order, then the rest of `visible` in their current order. Add `"order"` to the schema `required` list and to `properties`: `{"type":"array","items":{"type":"object","additionalProperties":false,"required":["file","reason"],"properties":{"file":{"type":"string"},"reason":text("Why read it here, under 8 words")}}}`. Add this line to `SYSTEM` after the `danger` line: `- order: every reviewed file once, in the order a reviewer should read them: the main behavior change first, each test right after the code it tests, config, generated files and docs last.`
- [ ] **Step 4: Run the tests.** Expected: PASS. Also run `cargo test -p revisor-core`. Expected: PASS (the schema change breaks no existing test).
- [ ] **Step 5: Model.** Add the fields and methods. `refilter`: after `filter_files`, if `ai_order` and `reading_order` is not empty, `self.visible = ai_review::reading_order(&self.files, &self.visible, &self.reading_order)`. `set_files`: set `selected` from `self.visible.first()` after `refilter`, not from `files.first()`. `set_reading_order` and `set_ai_order` store the value and call `refilter`.
- [ ] **Step 6: Sync from the panel.** Add `fn sync_order(&self, cx: &mut Context<Self>)` to `AiPanel`. It uses `review.order` when `is_current`, otherwise an empty `Vec`, and calls `set_reading_order` only when the value differs from `model.reading_order`. Call it in the load callback, in the `Finished` handler and in the T1 model observer (so a new head clears a stale order).
- [ ] **Step 7: Files tab.** Next to the list/tree view button, add the ghost xsmall button `AI order`: `.selected(model.ai_order)`, disabled when `reading_order` is empty, tooltip `Sort files in the AI reading order`, click `set_ai_order(!ai_order)`. In the flat list, show `order_reason(path)` as a muted, truncated suffix on the same row, so all rows keep the same height. The tree view keeps path order. Hide the Review focus block (`files.rs:530`) when `reading_order` is not empty.
- [ ] **Step 8: Verify.** Run `cargo check -p revisor-app`. In the app, run an AI review. The file list and the diff follow the AI order, the reasons show, and `u` / the T4 button follow the new order. Turn off **AI order**; the order goes back to the provider's order. Type a filter; hidden files stay hidden. A PR whose saved review is outdated uses the provider's order.
- [ ] **Step 9: Commit** `feat(ai-review): sort files in an AI reading order`.

### Task 7: Remove the Related section from the Files tab

**Files:**
- Modify: `crates/revisor-app/src/screens/pr_detail/files.rs` (doc comment :1, import :3, fields :51-55, reset :81-86, init :130-134, `related_section` :346-430 and its call site)
- Delete: `crates/revisor-app/src/screens/pr_detail/related.rs`
- Modify: `crates/revisor-app/src/screens/pr_detail/mod.rs` (`mod related;` :11)
- Modify: `docs/superpowers/plans/2026-10-18-related-files.md` (add one top line: `Superseded: the Related section was removed by 2026-10-18-guided-review-flow.md Task 7. The t key stays.`)

**Interfaces:**
- Keeps: `files::counterpart` (`files.rs:28-35`) and the `t` / `GoToRelatedTest` binding.

- [x] **Step 1: Delete the section**, its fields, its reset code and `related.rs`. Remove imports that become unused (`Relation`, `related` module). `related_files` stays imported because `counterpart` uses it.
- [x] **Step 2: Verify.** Run `cargo check -p revisor-app`. Expected: no errors and no unused warnings. In the app, the Files tab ends at the file list, and `t` on a source file still jumps to its test.
- [ ] **Step 3: Commit** `refactor(pr-detail): remove the Related section from the Files tab`.

### Task 8: Tidy-ups

**Files:**
- Modify: `crates/revisor-app/src/screens/pr_detail/files.rs:490`
- Modify: `crates/revisor-app/src/screens/settings.rs:340`
- Modify: `crates/revisor-app/src/screens/pr_detail/mod.rs:421` and `crates/revisor-app/src/screens/pr_detail/diff.rs` (`toolbar` :1066-1140); read `commits.rs` first

- [ ] **Step 1: Copy.** Change `"{reviewed} / {total} proofed"` to `"{reviewed} / {total} viewed"`. Change the page title `"expritrimental"` to `"Experimental"`.
- [ ] **Step 2: Move the commit selector.** Delete it from `sidebar()` (`mod.rs:421`). In the diff `toolbar`, render `commits::CommitSelector::new(self.model.clone())` after the `{n} files` eyebrow, when `!model.commits.is_empty()`. If `CommitSelector` renders a full-width row, make it a compact xsmall dropdown button in `commits.rs`, so the toolbar height does not change.
- [ ] **Step 3: Verify.** Run `cargo check -p revisor-app`. In the app, the tabs are at the top of the sidebar, the commit selector in the toolbar still filters the diff by commits, and the Settings page is named "Experimental".
- [ ] **Step 4: Commit** `chore(pr-detail): tidy labels and move the commit selector to the diff toolbar`.

### Task 9 (optional, after trial feedback): AI findings on their diff lines

Run this task only if trial users say that the Overview list is not enough (spec, Open question 2).

**Files:**
- Modify: `crates/revisor-app/src/screens/pr_detail/state.rs` (`PrEvent` :53-62, `PrModel` fields)
- Modify: `crates/revisor-app/src/screens/pr_detail/diff.rs` (`enum Row` :44-57, row build `anchored` :255-300, `step_thread` :473, row render)
- Modify: `crates/revisor-app/src/screens/pr_detail/ai.rs` (`sync_order` from T6, `mark_done` and `draft` from T3)

**Interfaces:**
- Consumes: T3 `Review.done` / `mark_done`; T6 `sync_order`.
- Produces: `PrModel.ai_findings: Vec<(usize, Comment)>` (open, located findings, by comment index). `PrEvent::AiFinding { index: usize, draft: bool }`. `Row::AiFinding(usize /*file*/, usize /*comment index*/)`.

- [ ] **Step 1: Sync.** Rename `sync_order` to `sync_model` and make it also set `ai_findings`: the open comments for which `ai_review::in_diff` is true, empty when the review is not current.
- [ ] **Step 2: Rows.** In the row build, after a line's threads, push `Row::AiFinding(fi, index)` for each finding on `(side, line)`. Render a muted row: `AI · {severity} · {axis}` stamps, the first line of the body, and xsmall buttons **Add comment** (emits `AiFinding { index, draft: true }`) and **Dismiss** (`draft: false`). `AiPanel` subscribes to model events and calls `draft` or `mark_done`.
- [ ] **Step 3: Keys.** `step_thread` (`n` / `N`) also stops on `Row::AiFinding` rows.
- [ ] **Step 4: Verify.** Run `cargo check -p revisor-app`. In the app, findings show under their lines, **Add comment** opens the prefilled form, **Dismiss** removes the row and lowers the Overview count, and `n` stops on findings.
- [ ] **Step 5: Commit** `feat(pr-detail): show AI findings on their diff lines`.

---

## Not planned

- Size hint (spec #10): the evidence is weak (vendor study). Add it only if trial users ask.
- Sequence diagrams, effort score, chat, tour mode, separate review map: the spec rejects them (roadmap decision 2026-10-18).
- Removing the TypeSafe features: wait for usage evidence (roadmap R5-01).

## Decisions made here (change them before you run a task if you disagree)

- Overview is the default tab even when the PR has no description and no review. It then shows `No description.` and the **Run AI review** prompt (spec Open question 1).
- Dismissals stay local, in the saved review JSON. Nothing is posted (spec Open question 3).
- The reading order comes from the existing AI call, not a local rule (spec Open question 4). Without a current review, the provider's order is used.
- With no current AI review, the next-step button stays **AI review**. A reviewer who does not want AI uses `u` and **Review** as today.
