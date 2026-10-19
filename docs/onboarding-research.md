# Onboarding research

Research date: 2026-10-19. Goal: a new user can try every feature in about two minutes, with no setup.

## Findings

| # | Finding | Source |
|---|---|---|
| 1 | In a test with 70 users, people who watched a tutorial did no better on tasks than people who skipped it (91% vs. 94%). | [NN/g](https://www.nngroup.com/articles/mobile-tutorials/) |
| 2 | 72-74% of users finish product tours of 3-4 steps. Only 16% finish tours of 7 or more steps. Tours that the user starts are finished more often (67%) than tours that start by themselves after a delay (31%). | [Chameleon](https://www.chameleon.io/blog/mastering-product-tours) |
| 3 | Checklists are finished less often than tours: 19.2% on average. Tick an item only when the user does the action, not when they see it. | [Userpilot](https://userpilot.com/blog/?p=197841), [Appcues](https://docs.appcues.com/checklist-best-practices) |
| 4 | Users learn by doing real tasks on sample data in the real UI. Examples: the GitHub Desktop tutorial repository, the Linear demo workspace, and the CodeRabbit sample PR with planted bugs. | [GitHub](https://github.blog/2019-10-02-get-started-easier-with-github-desktop-2-2/), [Linear](https://linear.app/docs/start-guide.md), [CodeRabbit](https://docs.coderabbit.ai/getting-started/quickstart/) |
| 5 | A demo must use the same code as real data, must be clearly labeled, and must use bundled offline data. Use the same fixtures as test data, so that the demo does not become outdated. | [rajiv.com](https://rajiv.com/blog/2026/04/14/demo-mode-as-a-pattern-for-software/) |
| 6 | Ask for access only when a feature needs it. Warp removed its required login so that users can try the product first. | [Warp](https://www.warp.dev/blog/lifting-login-requirement), [Uxcel](https://uxcel.com/lessons/ask-permission-best-practices-607) |
| 7 | Teach shortcuts while the user works. Do not make users learn all the shortcuts before they start. Let users open the tutorial again later. | [Superhuman](https://help.superhuman.com/article/478-superhuman-command), [Linear](https://linear.app/changelog/2019-05-09-first-time-user-experience) |
| 8 | Things to avoid: tours that users must complete, Skip buttons that are hard to find, tips that come back after the user closes them, and questionnaires at sign-up. | [Chameleon](https://chameleon.io/benchmark-report) |

## What Revisor does

- The sign-in screen has **Try a sample pull request**. Use it once, and there is nothing to set up.
- The sample is served by `DemoProvider`. It uses the same `GitProvider` interface as GitHub, so every screen runs its real code. The fixtures are also test data (`cargo test -p revisor-core demo`).
- The sample PR is a small change that looks real. It has planted bugs, a rename, a deleted file, a lockfile, three commits, an open thread and a resolved thread.
- The AI review is a recorded review. It runs with one click, and no CLI is needed.
- The status bar shows a **SAMPLE** label and a 5-step **Getting started** checklist. It does not open by itself. Each step gets a tick from the data that its action makes.
- **Connect GitHub** closes the sample and opens sign-in. **Start over** resets the sample.
- No tour opens when the app starts. The existing status-bar tips and the `?` overlay teach the shortcuts.

## Not done yet

- A way to open the sample while you are signed in. Add it if users ask for it.
- Local first-run milestones (sample opened, first real review) for roadmap item R5-03.
