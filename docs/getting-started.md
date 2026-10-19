# Getting started

Try every Revisor feature on a sample pull request first. You do not need a GitHub account or an AI CLI.

## Try the sample

1. Start Revisor: `cargo run -p revisor-app`.
2. On the sign-in screen, click **Try a sample pull request**.
3. Click **Getting started** in the status bar. Do the steps in order. Each step gets a tick when you do it.

| Step | How |
|---|---|
| Open pull request #42 | Select it in the list and press Enter. |
| Run the AI review | Open the Overview tab and click **Run AI review**. The sample replays a recorded review. |
| Mark a file viewed | Tick a file in the Files tab, or use the shortcut in the status bar. |
| Comment on a line | Click **Add comment** on an AI finding, or click a line in the diff. |
| Submit your review | Click **Review** in the toolbar, then Comment, Approve or Request changes. |

Things to try after the checklist:

- Use the commit selector in the diff toolbar to see one commit at a time.
- Press `t` to jump between `src/coupons.ts` and its test.
- See that `package-lock.json` starts collapsed.
- Reply to the open thread, or resolve it.
- Filter the PR list by label, author or AI review status.
- Press `?` for all shortcuts.

## What the sample does

- The sample repository `revisor-demo/shop` and its PRs come with the app. Revisor needs no network for them.
- Comments, reviews and resolved threads stay in memory. Nothing goes to GitHub.
- The AI review is a recorded review. No Claude Code or Codex run, and no data goes to Anthropic or OpenAI.
- **Start over** in the checklist resets all of it. When you quit Revisor, the sample ends. Next time it starts fresh.

## Use your own repositories

Click **Connect GitHub** in the status bar. This closes the sample and opens sign-in. See [GitHub OAuth setup](github-oauth.md).

The AI review needs [Claude Code](https://docs.anthropic.com/en/docs/claude-code) or [Codex](https://github.com/openai/codex), installed and signed in. Revisor finds the CLI itself. You can pick the CLI, model and effort next to the run button.

## Change the sample

The fixture files are in `crates/revisor-core/src/demo/sample/`:

- `base/` has the files on `main`.
- `prNN-K/` has the files that commit K of PR NN changes.
- `review.json` has the recorded AI review.

The PRs, commits and comment threads are in `crates/revisor-core/src/demo/mod.rs`.

`cargo test -p revisor-core demo` checks that every recorded finding and thread points at a line in the diff. Run it after you change a fixture.
