# Revisor product roadmap

Last updated: 2026-10-17

## Current focus

- **Goal:** Help individual developers understand changes and navigate GitHub reviews faster.
- **Product promise:** Understand the change. Keep your place. Finish the review.
- **First users:** Developers who review multi-file pull requests and move between GitHub and an editor to understand the code.
- **Current users:** Only the project owner, as last reported on 2026-10-11.
- **Current stage:** Release 0 — prepare the first external trial.
- **Next action:** R0-01 — verify installation and a complete review on a clean user setup. Record each obstacle before inviting trial users.
- **Primary task in progress:** R1-04 — pasted PR URL opening; manual app check pending.

## How to track progress

Use these status values:

| Status | Meaning |
|---|---|
| Not started | Work or acceptance verification has not started. Existing code may still be present. |
| In progress | This is the active task. |
| Blocked | Record the cause and the action needed to remove it in the evidence cell. |
| Done | The task meets its acceptance requirement and has dated completion evidence. |

1. Fix installation, sign-in, lost-work, and submission failures before adding features.
2. Otherwise, select the first unfinished task in the current release.
3. Keep one primary implementation task in progress. Recruitment and documentation may proceed in parallel.
4. Keep task IDs stable. Add new IDs rather than renumbering old tasks.
5. Record a date and a test result, user observation, or relative document link in the evidence cell. Code alone does not prove the user workflow works.
6. After each completed task, update Current focus and the progress log.
7. Advance a release only when its acceptance checklist is complete. Record any change to this rule in the decision log.

Do not put credentials, private source code, or private review comments in this document. Use anonymous tester labels in observations.

## Existing capabilities

These capabilities are implemented in code; user verification is pending. This inventory is not a completed-release claim.

| Capability | Repository evidence | Remaining verification |
|---|---|---|
| Keyboard navigation | Commit `e8516fa`; native PR-detail bindings for changes, threads, unreviewed files, and viewed state | Check complete workflows and input focus; do not rebuild existing commands |
| GitHub CLI sign-in | Commit `3ad73fe` | Check sign-in on a clean user setup |
| Large diffs loaded on demand | Commit `3586142` | Check responsiveness and clear loading states on large PRs |
| Diff search, file filters, commit selection, comments, and review submission | Native PR-detail modules in `crates/revisor-app/src/screens/pr_detail/` | Check an end-to-end review, including failure cases |
| AI file suggestions and semantic search | Native files and semantic panels; shared core modules | Check usefulness, consent, revision validity, and analysis limits |

The README describes an offline-capable app. Verify the actual offline limits before making release claims. Existing OAuth setup documentation includes Tauri commands; native setup instructions need verification under R0-02.

## Release 0 — Usable external trial

**Outcome:** Another developer can install Revisor and finish a real review without help.

| ID | Task | Status | Completion evidence |
|---|---|---|---|
| R0-01 | Check installation, sign-in, PR opening, comments, and review submission on a clean user setup; record and fix release-blocking failures | Not started | No clean-setup observation recorded |
| R0-02 | Provide an installable native build for the current supported platform and verified sign-in/access instructions | Not started | Native distribution and setup verification pending |
| R0-03 | Document supported workflows, offline/network limits, and optional AI data transfer | Not started | User-facing limits not verified |
| R0-04 | Provide safe problem reporting with app version and diagnostics, excluding credentials and source code | Not started | Reporting flow not verified |
| R0-05 | Recruit five developers; observe one review in their usual tool and their first Revisor review | Not started | No external trial users reported |

### Acceptance checklist

- [ ] Five developers can attempt a real review using the supplied build and instructions.
- [ ] Each can complete the core review flow without guidance.
- [ ] Known installation, sign-in, lost-work, and submission failures are resolved.
- [ ] Network and AI privacy limits are clear before users rely on them.

Recruitment and documentation can proceed while release-blocking defects are fixed.

## Release 1 — Faster navigation

**Outcome:** Reviewers move through a PR without repeated scrolling or losing their location.

| ID | Task | Status | Completion evidence |
|---|---|---|---|
| R1-01 | Verify and complete keyboard movement between files, changed blocks, unreviewed files, and unresolved threads | Not started | Bindings exist in `e8516fa`; acceptance verification pending |
| R1-02 | Add navigation history that returns to the prior file and line after search or comment jumps | Not started | Acceptance evidence not recorded |
| R1-03 | Make shortcuts discoverable through visible help and control labels | Not started | Acceptance evidence not recorded |
| R1-04 | Open a pasted GitHub PR URL with clear access, invalid-URL, and loading errors | In progress | 2026-10-17: URL parser and error-message unit tests pass; manual check in the app pending |
| R1-05 | Verify and improve large-PR responsiveness while diffs load | In progress | 2026-10-17: syntax colors for diffs over 1,000 lines move off the UI thread (20k lines measured 313 ms in release); inline loading/failure/retry, threads of unloaded files, and unsearched large files are marked; manual check on a large PR pending |

### Acceptance checklist

- [ ] Review shortcuts do not intercept typing in search fields or comment editors.
- [ ] Back navigation restores the correct file and line.
- [ ] Missing or loading diff content is clearly marked.
- [ ] At least three trial users complete two real reviews and report fewer navigation interruptions, supported by observation.

## Release 2 — Code context

**Outcome:** Reviewers find enough context to explain a change without repeatedly leaving Revisor.

| ID | Task | Status | Completion evidence |
|---|---|---|---|
| R2-01 | Show surrounding source beside the diff at the selected revision, with old/new versions where needed | Not started | Acceptance evidence not recorded |
| R2-02 | Preserve review location when context opens or closes | Not started | Acceptance evidence not recorded |
| R2-03 | Provide quick access to related changed files and tests; extend existing search rather than add a separate search flow | Not started | Acceptance evidence not recorded |
| R2-04 | Show missing, unsupported, and unavailable context explicitly | Not started | Acceptance evidence not recorded |

### Acceptance checklist

- [ ] Context matches the selected commit, not the latest branch contents.
- [ ] Reviewers can return to the exact review location.
- [ ] Trial users explain the main behavior change and identify supporting code with fewer switches to their browser or editor.
- [ ] Observation checks understanding as well as speed.

Start with source context and changed-file relationships. Do not require full repository indexing.

## Release 3 — Guided review map

**Outcome:** Reviewers know where to start in a large PR and what to inspect next.

| ID | Task | Status | Completion evidence |
|---|---|---|---|
| R3-01 | Group related changes with a short behavior description and exact diff links | Not started | Acceptance evidence not recorded |
| R3-02 | Suggest a reading order with reasons and links to relevant changed tests | Not started | Acceptance evidence not recorded |
| R3-03 | Extend existing optional AI assistance; identify analyzed revisions, discard stale results, and list skipped files | Not started | Existing AI panels require task-specific verification |

### Acceptance checklist

- [ ] The normal file list remains available; AI does not hide low-ranked files.
- [ ] Suggestions link to supporting code and are not presented as proof of safety.
- [ ] Users understand what code will leave the device before an AI request.
- [ ] At least three trial users use the map on multiple PRs and identify a specific improvement in understanding.
- [ ] If the map only adds text without benefit, revise or remove it before expanding it.

## Release 4 — Reliable follow-up reviews

**Outcome:** Returning to a PR does not require repeating the first review.

| ID | Task | Status | Completion evidence |
|---|---|---|---|
| R4-01 | Restore review location and filters; preserve local unfinished comment text safely | Not started | Acceptance evidence not recorded |
| R4-02 | Show changes since the last review using the reviewed revision, not only a viewed-file flag | Not started | Acceptance evidence not recorded |
| R4-03 | Mark changed files for another check and explain unavailable comparisons after force-pushes | Not started | Acceptance evidence not recorded |
| R4-04 | Distinguish local drafts, GitHub pending comments, and submitted reviews; show submission failures accurately | Not started | Existing submission flow requires state and failure verification |

### Acceptance checklist

- [ ] Restart and connection loss do not silently lose unfinished work.
- [ ] New commits do not silently retain an incorrect reviewed state.
- [ ] Force-pushes produce a clear comparison result or a clear limitation.
- [ ] Failed submissions are never shown as successful.

## Release 5 — Adoption and payment

**Outcome:** Confirm repeat use before investing in a larger product.

Observation starts in Release 0 and continues throughout the roadmap. Do not wait for all features before checking retention.

| ID | Task | Status | Completion evidence |
|---|---|---|---|
| R5-01 | Track first successful reviews, repeat use, and the task users would miss most | Not started | No external observations recorded |
| R5-02 | Publish a short real-workflow demonstration and invite a second group of similar developers | Not started | Start after repeat use is established |
| R5-03 | Test whether new users can start without a call; improve installation and guidance where needed | Not started | No independent-start observations recorded |
| R5-04 | Interview repeat users, estimate operating costs, and test a simple individual paid offer | Not started | Start after repeat use; price not yet selected |

### Validation targets

| Measure | Target | If missed |
|---|---|---|
| First successful review | All five trial users can complete one | Fix onboarding and completion barriers |
| Repeat use | At least three of five use Revisor in each of three consecutive weeks when they have reviews to perform | Find why they return to another tool before expanding scope |
| Specific benefit | Repeat users name a task Revisor improves | Focus work on the strongest observed benefit |
| Independent start | Second-group users can start without a call | Improve installation and guidance |
| Willingness to pay | Repeat users accept a real paid offer | Reassess value and pricing before adding billing complexity |

These are small-sample validation targets, not proof of market demand. Record the sample and observation period. Do not treat a week with no reviews to perform as proof of rejection.

### Acceptance checklist

- [ ] Repeat-use observations include dates and anonymous tester labels.
- [ ] Product decisions use direct observation and consent-based counts, not downloads or stars alone.
- [ ] Usage analytics exclude source code, comment bodies, repository names, and credentials.
- [ ] A paid-offer decision records the price tested and actual responses, not only stated interest.
- [ ] Any paid cloud AI cost is considered separately from the essential local review experience.

## Delivery and verification rules

- Use the native Rust app as the primary product. Keep the older Tauri interface as a fallback; do not expand both interfaces.
- Keep GitHub as the only provider until repeat use is established.
- Make navigation and context useful without AI. Do not approve code automatically.
- Use release gates rather than fixed dates until capacity is known.
- Create a separate implementation plan for each feature. This roadmap does not define new public APIs or database migrations.
- Keep changes focused on the native PR-detail interface, shared state, GitHub provider, and cache as needed. Avoid unrelated refactors.
- Test affected logic and persistence narrowly. Verify UI behavior with real PRs, including large diffs, renamed files, binary files, and restricted repositories.
- Check interrupted requests, expired sign-in, restart, and failed submission where relevant.
- Consult Jean run-environment information before testing a running app. Record the actual command and any URL/port used.
- Compare equivalent review tasks with each tester's usual workflow. Check comprehension as well as navigation effort.

## Deferred work

| Work | Why deferred | Reconsider when |
|---|---|---|
| GitLab, Bitbucket, and additional platforms | Integration and support costs distract from the first workflow | Active users repeatedly need them |
| Team dashboards and merge queues | Require team adoption and a different buying decision | Multiple users in the same organization request shared work |
| General AI chat and automatic fixes | Do not directly prove the main product benefit | Observations show a need beyond linked explanations |
| Full repository indexing and broad language analysis | Adds technical scope before context value is proven | Simple source context fails repeated real tasks |
| Extensive customization and large architecture rewrites | Delay feedback | A measured usability or delivery problem requires them |

## Decision log

| Date | Decision | Reason |
|---|---|---|
| 2026-10-17 | Target individual GitHub reviewers; prioritize navigation and understanding | User-selected audience and main review problems |
| 2026-10-17 | Native app first; optional AI assistance | Keep development focused and the core workflow independent of AI |
| 2026-10-17 | Seek five external trial users before expanding scope | Only the owner is currently reported to use the app |
| 2026-10-17 | Track acceptance separately from code presence | Existing implementation does not prove external usability |

## Progress log

| Date | Task or release | Evidence and result | Next action |
|---|---|---|---|
| 2026-10-17 | Roadmap setup | Read repository documentation and recent history. Keyboard navigation was added in `e8516fa`; no external trial evidence was supplied. Product tasks remain open. | Start R0-01: clean-setup review verification |

When work is completed, append a dated row and update its task status, evidence, and Current focus. Do not replace earlier observations.
