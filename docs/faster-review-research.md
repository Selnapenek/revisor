# Faster code review in Revisor

## Question

The PR detail screen feels crowded, and it does not show what to do next. A reviewer should probably start with the PR description and the AI review, then read the code alone or with AI help. What does research and current tooling say makes review faster, and what should Revisor change, hide or remove to support that order?

## Current layout map

All paths are relative to `crates/`. Line numbers refer to commit `ad489d0`.

### What is on screen at the same time

| Area | Contents | Default | Source |
| --- | --- | --- | --- |
| Header | `← PRs`, **Sidebar** toggle, title and meta line, **Open on GitHub**, **AI review**, **Review** / **Finish review (n)**, **Description** toggle | Always shown. Every button is a ghost button. **Review** becomes primary only after a pending review exists. | `revisor-app/src/screens/pr_detail/mod.rs:244-345`, `:325-329` |
| Left sidebar (260 px) | Commit selector, then tabs **Files n**, **Conversations n**, **AI**, and **Semantic** when opt-in is on | Open, with the **Files** tab selected | `mod.rs:59`, `:222-224`, `:410-444` |
| Centre | Diff toolbar (file count, collapse or expand all, Back, search, Split/Unified) and the diff, where each file header has a **Viewed** checkbox | Always shown | `diff.rs:1066-1140`, `:847-899` |
| Right pane (340 px) | PR description as Markdown | **Open** whenever the PR has a body | `mod.rs:60`, `:225`, `:362-375`, `:516-525` |

Files tab, from top to bottom: an experimental **Review focus** box, a filter input with view and glob buttons, the glob fields, an "n / m proofed" progress bar, the file list with a viewed checkbox per row, and a **Related** section (`files.rs:433-537`, `:346-430`).

AI tab, from top to bottom (`ai.rs:472-530`, `:365-469`):

1. Backend, model and effort pickers, the "Runs … effort" line, and **Run AI review** / **Run again**.
2. Summary, Evidence, Merge danger, Spec, and Standards check (with three sub-blocks).
3. Comments in three groups by axis (Bugs, Spec, Standards), each with **Add comment** and **Dismiss**.
4. Not reviewed files.

Conversations tab: a **Timeline / Findings** segmented control. **Findings** appears only when opt-in is on (`conversations.rs:528-553`).

**What the user sees first:** the file list on the left, the first file's diff in the centre, and the description on the right. The first file is just whatever the provider returned first (`state.rs:250-252`). Nothing reorders the files. The AI result is one tab away, even when a background run from the PR list has already finished (`pr_list.rs:352-356`, `:586-601`).

### AI and helper features

| Feature | What it does | Backend | Opt-in | Source |
| --- | --- | --- | --- | --- |
| One-click AI review | Recap (summary, evidence, merge danger, spec, standards) plus at most 10 line comments on three axes. Lock and generated files are skipped. Results are saved per PR and marked outdated on a new head. | Claude Code / Codex CLI | No. Runs when the user clicks it. | `revisor-core/src/ai_review.rs:30-49`, `:147`, `:267-278`; `ai.rs:185-208` |
| Background AI review from the PR list | Same review, started from the list, with a status stamp | Same | No | `pr_list.rs:352-356`, `:586-601` |
| Review focus | Scores each file on 4 topics (access, data, interface, behavior) and lists the files worth a closer look | TypeSafe API key | Yes | `focus.rs:139-145`, `:266`; `files.rs:530`, `:539-760` |
| Findings ranking | Ranks existing GitHub comments from selected AI-bot authors by impact and confidence | TypeSafe | Yes | `rank_findings.rs:355`; `conversations.rs:411-526` |
| Semantic search | Searches the diff in plain language; hits jump to their hunk | TypeSafe | Yes | `semantic.rs`; `pr_detail/semantic.rs:188-234` |
| Related files | Name-based matching (test, source, same stem, mentions, same folder) in a Files-tab section, plus the `t` key | Local | No. Always shown. | `revisor-core/src/files.rs:314-339`; `files.rs:346-430`; `mod.rs:79`, `:475-479` |

The TypeSafe features are behind one switch, which is off by default (`experimental.rs:7-20`, `settings.rs:340-346`). The settings page title is spelled `"expritrimental"` (`settings.rs:340`).

### Sources of overload

| # | Problem | Evidence in code | Effect |
| --- | --- | --- | --- |
| O1 | Three panes compete from the start: sidebar, diff and description | `mod.rs:224-225`, `:493-526` | The diff gets the leftover width, and the description sits beside code the reviewer has not reached yet |
| O2 | No primary next step. Six header controls look the same. | `mod.rs:251-344` | The reviewer has to decide what to do first |
| O3 | Understanding comes after navigation. The Files tab and the first file open first; the AI recap is hidden in a tab. | `mod.rs:222`; `state.rs:252` | The opposite of the "broad view first" advice (see Evidence) |
| O4 | AI tab order: settings first, then five prose sections, with the actionable comments last | `ai.rs:483-506`, `:409-458` | The things to act on are below the fold. Pickers repeat Settings (`settings.rs:276`). |
| O5 | AI comments appear only in the sidebar, not on the diff line | `ai.rs:306-359`; the diff shows them only after **Add comment** (`diff.rs:149-153`) | Attention splits between the list and the diff |
| O6 | Four separate "where to look" features: AI comments, Review focus, Findings ranking, Related. Semantic search sits beside them. | Table above | Overlapping answers in different places with different confidence labels |
| O7 | File order is the provider's order, with no reading order | `state.rs:250-252`; the shortcut note at `ai_review.rs:19-20` | Later files get less attention (see Evidence) |
| O8 | Triage is not saved. **Dismiss** is an in-memory set. | `ai.rs:48-49`, `:83`, `:352-356` | Dismissed comments come back after a restart |
| O9 | Generated and lock files are skipped by AI but still expanded in the diff | `ai_review.rs:267-278`; `state.rs:237` collapses only viewed files | Noise in the reading path |
| O10 | Small inconsistencies: "proofed" on the progress bar versus "Viewed" on checkboxes; a rarely used commit selector above the tabs | `files.rs:490`, `diff.rs:890`; `mod.rs:421` | Extra words and controls to read past |

## Evidence

### What makes review slow or ineffective

- **Understanding is the core task.** At Microsoft, finding defects was the top stated reason for review, but it delivered less than expected. Understanding the change was central, and tools supported it poorly. [Bacchelli & Bird 2013](https://www.microsoft.com/en-us/research/?p=164195), [ICSE page](https://2013.icse-conferences.org/content/expectations-outcomes-and-challenges-modern-code-review.html)
- **Read the description, then the main part, then the rest.** Google's reviewer guide says: Step 1, "take a broad view of the change", starting with the description. Step 2, "examine the main parts" first, and send design comments right away. Step 3, read the rest "in an appropriate sequence". [Google eng-practices](https://google.github.io/eng-practices/review/reviewer/navigate.html)
- **Google review is lightweight and changes are small.** Google's study covers 9M reviewed changes, 12 interviews and 44 survey responses. [Sadowski et al. 2018](https://research.google/pubs/modern-code-review-a-case-study-at-google/). Critique says "most changes are small (fewer than 100 lines)". [SWE at Google, ch. 19](https://abseil.io/resources/swe-book/html/ch19.html). *Not verified in the paper:* median approval under 4 hours, which appears only in [student slides](https://plg.uwaterloo.ca/~migod/846/current/summaries/10-Youssef-GoogleCodeReview-slides.pdf).
- **Size and pace limits (vendor study, Cisco team):** review 200–400 lines at a time, stay under 500 lines per hour, and expect performance to drop after about 60 minutes. Author annotations before review help. [SmartBear](https://smartbear.com/learn/code-review/best-practices-for-peer-code-review/). The page gives both 60 and "60 to 90" minutes. This is a vendor source, not peer-reviewed.
- **Smaller changes are reviewed better.** Effectiveness was "significantly larger for small changes". The same experiment could not confirm that the order of change parts affects efficiency. [Baum et al. 2019 (UZH)](https://www.merlin.uzh.ch/publication/show/20243)

### File order and grouping

- **Position bias.** In 219,476 PRs, files placed earlier got more comments. In an experiment with 106 participants, the odds of finding one defect type were 64% lower when its file came last. [Fregnan et al. 2022](https://arxiv.org/abs/2208.04259)
- **Attention dilution.** Across 330k PRs, latent bug rate rises with file position (56.7% at position 1, 61.5% at position 30). Each extra file lowers the odds of any comment by about 8.7%. The authors say alphabetical order "is not a neutral interface default" and propose risk-aware ordering, dependency grouping and per-file coverage indicators. [Rahman et al. 2026 (preprint)](https://arxiv.org/abs/2609.22610)
- **Developers dislike alphabetical order.** Only about 10% of 1,355 developers find it optimal, and 66% asked for dependency-aware or customizable ordering. [Rahman et al., ICSE 2026](https://conf.researchr.org/details/icse-2026/icse-2026-research-track/310/Breaking-the-Alphabet-Rethinking-File-Ordering-in-Code-Review)
- **The effect held on IntelliJ IDEA.** Ordering by diff size placed effort-heavy files higher. [Bagirov et al. 2023 (preprint)](https://arxiv.org/abs/2306.06956)
- **Group by relatedness.** The best order is "mainly a grouping of change parts by relatedness". [Baum et al., ICSME 2017](https://www.merlin.uzh.ch/publication/show/15587)
- **Reading tests first is not a general win.** It found more test defects, the same number of production defects and fewer maintainability issues. Most developers prefer to read production code first. [Spadini et al., ICSE 2019](https://serg.ewi.tudelft.nl/publications/test-driven-code-review-an-empirical-study/). Pairing tests with their code is safer than forcing tests first. This is an inference.

### Throughput and "what next"

- **Meta.** Time In Review at the 75th percentile tracks dissatisfaction, and "eyeball time" guards against rubber-stamping. **Next Reviewable Diff** queues the next diff and raised review actions per day by 17%. Nudgebot cut Time In Review by 7%. [Meta Engineering 2022](https://engineering.fb.com/2022/11/16/culture/meta-code-review-time-improving/). In a randomized test with over 30k engineers: −6.8% time in review and −9.9% time to first action. [Shan et al., FSE 2022](https://2022.esec-fse.org/details/fse-2022-research-papers/26/Using-Nudges-to-Accelerate-Code-Reviews-at-Scale)
- **Gerrit's attention set** tells each person whose turn it is ("Your turn" section, bold names). [Gerrit docs](https://gerrit-review.googlesource.com/Documentation/user-attention-set.html)
- **Critique's scoring panel** shows approvals still needed and unresolved comments, so authors see "what they still need to do". Analyzer findings appear in the diff and can become a comment with **Please fix**. [SWE at Google, ch. 19](https://abseil.io/resources/swe-book/html/ch19.html)

### How tools structure the flow

| Tool | Overview first | Reading aid | AI findings | Source |
| --- | --- | --- | --- | --- |
| GitHub | Conversation tab, then Files changed | **Viewed** collapses a file and unmarks it if the file changes; header progress bar; file tree and filter; `linguist-generated` files hidden by default | Copilot posts an overview comment plus inline suggested fixes. It does not count as approval by default and skips lock, log and SVG files. | [Review guide](https://docs.github.com/en/pull-requests/collaborating-with-pull-requests/reviewing-changes-in-pull-requests/reviewing-proposed-changes-in-a-pull-request), [Generated files](https://docs.github.com/en/repositories/working-with-files/managing-files/customizing-how-changed-files-appear-on-github), [Copilot review](https://docs.github.com/en/copilot/concepts/agents/code-review) |
| Graphite | Stack, then Description, then Discussion, then file tree and diff | `F` toggles the tree; generated files hidden; **Finish review** button at the top | Agent review status; **Generate** description | [PR page](https://graphite.com/docs/pr-page-overview) |
| CodeRabbit | Walkthrough comment at the top, collapsed by default | Changed-files summary with related changes merged into rows; effort score 1–5; sequence diagrams. Each section can be turned off. | Comments tagged by category and 5 severity levels | [Walkthroughs](https://docs.coderabbit.ai/pr-reviews/walkthroughs), [Review overview](https://docs.coderabbit.ai/guides/code-review-overview) |
| Reviewable | n/a | Reviewed state per file per revision; **Show Diffs to Review** picks what to read next; finished files fade | n/a | [Files docs](https://docs.reviewable.io/files.html) |
| Gerrit / Critique | Dashboard "Your turn" | Attention set; scoring panel | Analyzer findings in the diff | Above |
| Microsoft (internal) | AI-written PR description | n/a | Line comments with a category; Q&A in the thread; **apply change** is explicit. 10–20% faster median PR completion in early repos. | [MS Engineering 2025](https://devblogs.microsoft.com/engineering-at-microsoft/enhancing-code-quality-at-scale-with-ai-powered-code-reviews/) |

### AI-guided review: precision beats volume

- **Google ML-suggested edits** were tuned for 50% precision; Google "traded quantity for quality" and acceptance went up. Wrong suggestions "reduce the developers' trust". A prominent **Show ML-edit** button doubled previews from about 20% to about 40%. [Google Research 2023](https://research.google/blog/resolving-code-review-comments-with-ml/)
- **Google AutoCommenter** (best-practice comments from an LLM) had a positive effect on workflow; the paper discusses deploying it to tens of thousands of developers. [Vijayvergiya et al. 2024](https://arxiv.org/abs/2405.13565)
- **Caution: AI review can slow PRs down.** At Beko, 73.8% of AI comments were resolved, but average PR closure time rose from 5h52m to 8h20m. Faulty and irrelevant comments were the stated drawbacks. [Cihan et al., ICSE SEIP 2025](https://arxiv.org/abs/2412.18531)
- **What reviewers need to know**, from 900 review comments: seven categories, such as how changed methods and variables are used. [Pascarella et al., CSCW 2018](https://research.tudelft.nl/en/publications/information-needs-in-contemporary-code-review)

### Not verified or not covered

- Meta Phabricator "review by intent": no primary source found.
- Linear-style "next action" UX, and the review UIs in Zed, JetBrains and VS Code: not researched.
- Sourcegraph/Cody review and GitLab Duo review: not researched. GitLab layout is covered in `docs/review-context-research.md`.
- Graphite's AI reviewer (Diamond, now Graphite Agent): placement of comments is not documented on the page checked.

## Recommendations

Effort: **S** is under half a day, **M** is 1–3 days, **L** is longer. Ordered by value for effort.

| # | Change | Reuses | Evidence | Effort | Hide, collapse or remove |
| --- | --- | --- | --- | --- | --- |
| 1 | **Make "Overview" the first tab and the default.** Rename the AI tab to Overview. Put the PR description at its top (collapsible), then the AI recap. Open on this tab, not Files. | `description()` `mod.rs:363-375`; `AiPanel` `ai.rs:365-469`; `tab:` default `mod.rs:222` | Google Step 1; Bacchelli & Bird; Graphite and GitHub put the description first | S | **Remove the right description pane** and the header **Description** toggle (`mod.rs:332-344`, `:516-525`). The diff gets 340 px back. |
| 2 | **Reorder the Overview:** Summary, then a single line of stamps (door, blast radius, spec status), then Findings. Put Evidence and Standards check under a closed **Details** disclosure. | `ai.rs:409-457` | CodeRabbit collapses its walkthrough; Critique stamps; Google precision lesson | S | Move the backend, model and effort pickers out of the panel. They already exist in Settings (`settings.rs:276`). Keep the one "Runs … effort" line. |
| 3 | **One findings list for triage:** sort by severity and show the axis as a stamp instead of three groups. Show "n of m left". Save dismissals and drafts in the saved `Review` JSON so they survive a restart. | `AXES` `ai_review.rs:147`; `done` `ai.rs:48-49`; `ai_review::save`; add a field with `#[serde(default)]` like `head_sha` | CodeRabbit severity; Cihan et al. (noise slows PRs, so dismissing must be quick and stick) | S–M | Remove the three axis headers and the "None found" row per axis. Keep one disclaimer line. |
| 4 | **One primary header button for the current step:** **AI review**, then **Findings n left**, then **Next unviewed file**, then **Finish review (n)**. The others stay ghost. | Header buttons `mod.rs:311-331`; `navigate_unreviewed` `state.rs:408`; `pending_count` | Gerrit attention set; Critique scoring panel; Meta Next Reviewable Diff | S | Remove the **Sidebar** text toggle from the header, or make it icon-only. |
| 5 | **Collapse generated and lock files by default** in the diff, using the same list the AI skips. | `GENERATED` / `skip_reason` `ai_review.rs:267-278` (make public); `collapsed` init `state.rs:237` | GitHub `linguist-generated`; Graphite; Copilot excludes lock and SVG files | S | Expands on click as now |
| 6 | **AI reading order as a sort, not a map:** add one `order: [{file, reason}]` field to the existing AI schema. When present, the file list and diff use it, with an **AI order / A–Z** toggle. Show the reason as one muted line on the file row. Put each test after its source file. | `schema()` `ai_review.rs:150`; `set_files` / `refilter` `state.rs:250-265`; `Relation::Test` `files.rs:314-339` | Fregnan 64%; Rahman 8.7% per file; Baum grouping; Google Step 2; Spadini (pair tests, don't force tests first) | M | Hide **Review focus** (`files.rs:530`) when the AI order exists, since it answers the same question. |
| 7 | **AI comments on their diff line:** a muted marker row under the line with **Add comment** and **Dismiss**. Uses the same draft path. `n`/`N` already step through threads; add findings to that stepping. | `PrEvent::DraftComment` `diff.rs:149-153`; `ai_review::in_diff`; `step_thread` `diff.rs:473` | Critique findings in the diff; Microsoft line comments; avoids split attention (O5) | M | The Overview list stays as an index. It does not duplicate the bodies. |
| 8 | **Remove the Related section** from the Files tab. Keep the `t` key and the test pairing from #6. | `counterpart` `files.rs:28-35` | `docs/review-context-research.md` (repeats file navigation); YAGNI | S | Delete `related_section` (`files.rs:346-430`) and `related.rs` if nothing else uses it |
| 9 | **Smaller tidy-ups:** rename "proofed" to "viewed"; move the commit selector into the diff toolbar; fix the "expritrimental" title. | `files.rs:490`; `mod.rs:421`; `settings.rs:340` | GitHub uses "Viewed" | S | One fewer row above the tabs |
| 10 | **Size hint** (optional): show "N changed lines" on the Overview, and "consider two sessions" above about 400 lines. | `ProcessedFile.additions/deletions` | SmartBear (vendor; treat as weak evidence) | S | Skip if trial users ignore it |

**Not recommended now:** sequence diagrams, a 1–5 effort score, chat, a step-by-step "tour" mode, and a separate review map. The decision log says the user rejected the earlier review-map attempt because it "added steps without value" (`docs/roadmap.md`, decision 2026-10-18), and roadmap R3 requires that "the normal file list remains available". Recommendation #6 meets that by sorting the existing list instead of adding a view. Keep the TypeSafe features opt-in. Remove them later if trial users never turn them on.

## Proposed flow and wireframe

1. **Understand** (Overview tab, open by default): description, AI recap, the danger and spec stamps. If no review exists yet, the primary button is **AI review**. Do not run it automatically on open, because roadmap R3 requires that users know what code leaves the device before an AI request.
2. **Triage** (same tab): work down the severity-sorted findings with **Add comment** or **Dismiss**. The primary button shows **Findings n left**.
3. **Read the code**: the primary button becomes **Next unviewed file** (`u`), following AI order when available, otherwise A–Z. AI markers sit on their lines (#7). `v` marks a file viewed and collapses it, as now.
4. **Submit**: **Finish review (n)** opens the existing dialog (`review.rs`).

```
┌───────────────────────────────────────────────────────────────────────────────────────┐
│ ← PRs │ Retry uploads with backoff                        ↗  [ Findings 2 left ▸ ]    │
│       │ Open  #412 · alice · 2h · main ← retry-backoff                                │
├──────────────────────────────┬────────────────────────────────────────────────────────┤
│ Overview │ Files 3/12 │ Conv 4│ 12 files  ⌃⌄   AI order ▾          ← Back  ⌕  Split  │
│──────────────────────────────│────────────────────────────────────────────────────────│
│ ▾ DESCRIPTION                │ ▾ M src/upload/retry.rs     +48 −12        [ ] Viewed  │
│   Adds exponential backoff…  │   40    for attempt in 0..max {                        │
│                              │   41  +   let delay = base * 2u32.pow(attempt);        │
│ AI RECAP · Claude · a1b2c3d  │   ┆ AI · high · bug  Overflow past 31 attempts         │
│   Uploads retry up to 5×     │   ┆                   [Add comment] [Dismiss]          │
│   with doubling delay…       │   42  +   sleep(delay).await;                          │
│   two-way door · upload ·    │                                                        │
│   spec: matches              │ ▸ M src/upload/retry_test.rs  · tests retry.rs         │
│ ▸ Details (evidence, rules)  │ ▸ M src/client.rs             · calls new retry        │
│                              │ ▸ M Cargo.lock                · generated, collapsed   │
│ FINDINGS · 2 of 3 left       │                                                        │
│ ● high bug   retry.rs:41     │                                                        │
│ ● med  spec  client.rs:88    │                                                        │
│ ○ dismissed 1  (show)        │                                                        │
└──────────────────────────────┴────────────────────────────────────────────────────────┘
```

What disappears from today's default screen: the right description pane, the **Description** and **Sidebar** header buttons, the AI pickers, three axis headers, the Related section, and the commit selector row above the tabs.

## Open questions

1. Should Overview be the default even when the PR has no description and no AI review? The alternative is opening Files and showing a one-line prompt.
2. Inline AI markers (#7): are they worth the diff-row work, or is a severity-sorted list with jump links (#3) enough? Check with trial users before building #7.
3. Should dismissals stay local, or be recorded somewhere visible to the PR author? Local only is simpler and posts nothing.
4. Should the AI reading order (#6) come from the same AI call or from a local rule (main source files first, then tests, config, docs)? A local rule needs no AI and works offline. The cited evidence supports grouping and moving likely-main files up, not a specific algorithm.
5. Keep, merge or remove the TypeSafe features (Review focus, Findings ranking, Semantic) once #6 lands? There is no usage evidence yet. Roadmap R5-01 would supply it.
6. Measure success the way Meta does: time to finish a review plus a guardrail against rubber-stamping, such as files viewed and comments kept. The roadmap already asks for "how many suggested comments reviewers keep".

## Sources

- https://www.microsoft.com/en-us/research/?p=164195
- https://2013.icse-conferences.org/content/expectations-outcomes-and-challenges-modern-code-review.html
- https://research.google/pubs/modern-code-review-a-case-study-at-google/
- https://plg.uwaterloo.ca/~migod/846/current/summaries/10-Youssef-GoogleCodeReview-slides.pdf (secondary, unverified figures)
- https://google.github.io/eng-practices/review/reviewer/navigate.html
- https://abseil.io/resources/swe-book/html/ch19.html
- https://smartbear.com/learn/code-review/best-practices-for-peer-code-review/
- https://www.merlin.uzh.ch/publication/show/20243
- https://www.merlin.uzh.ch/publication/show/15587
- https://arxiv.org/abs/2208.04259
- https://arxiv.org/abs/2609.22610
- https://conf.researchr.org/details/icse-2026/icse-2026-research-track/310/Breaking-the-Alphabet-Rethinking-File-Ordering-in-Code-Review
- https://arxiv.org/abs/2306.06956
- https://serg.ewi.tudelft.nl/publications/test-driven-code-review-an-empirical-study/
- https://research.tudelft.nl/en/publications/information-needs-in-contemporary-code-review
- https://engineering.fb.com/2022/11/16/culture/meta-code-review-time-improving/
- https://2022.esec-fse.org/details/fse-2022-research-papers/26/Using-Nudges-to-Accelerate-Code-Reviews-at-Scale
- https://research.google/blog/resolving-code-review-comments-with-ml/
- https://arxiv.org/abs/2405.13565
- https://arxiv.org/abs/2412.18531
- https://devblogs.microsoft.com/engineering-at-microsoft/enhancing-code-quality-at-scale-with-ai-powered-code-reviews/
- https://gerrit-review.googlesource.com/Documentation/user-attention-set.html
- https://docs.github.com/en/pull-requests/collaborating-with-pull-requests/reviewing-changes-in-pull-requests/reviewing-proposed-changes-in-a-pull-request
- https://docs.github.com/en/repositories/working-with-files/managing-files/customizing-how-changed-files-appear-on-github
- https://docs.github.com/en/copilot/concepts/agents/code-review
- https://graphite.com/docs/pr-page-overview
- https://docs.coderabbit.ai/pr-reviews/walkthroughs
- https://docs.coderabbit.ai/guides/code-review-overview
- https://docs.reviewable.io/files.html
