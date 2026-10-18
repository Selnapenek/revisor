# Source context in code review

## Question

Why can a full source file beside a pull request diff feel awkward, and should it repeat change marks and a list of related changed files?

## Evidence from review tools

| Tool | Source context and versions | File navigation |
| --- | --- | --- |
| GitHub | The **Files changed** tab is the review surface. Reviewers can choose unified or split diff view and comment on diff lines. [GitHub review guide](https://docs.github.com/en/pull-requests/how-tos/review-pull-requests/reviewing-proposed-changes-in-a-pull-request) | A file tree and file filter take reviewers to a changed file in that same tab. [GitHub review guide](https://docs.github.com/en/pull-requests/how-tos/review-pull-requests/reviewing-proposed-changes-in-a-pull-request) |
| GitLab | The diff shows changed portions by default. Controls in the diff gutter add 20 unchanged lines before or after a hunk. Inline and side-by-side modes show old and new lines in the diff. A **View File @ $SHA** action opens the full file in a separate tab. [GitLab changes guide](https://docs.gitlab.com/user/project/merge_requests/changes/) | The file browser shows changed files as a tree or list and can be hidden. Reviewers can select a file there. [GitLab changes guide](https://docs.gitlab.com/user/project/merge_requests/changes/) |
| Gerrit | The diff screen shows old and new versions side by side. Reviewers can set the number of context lines, expand skipped lines in place, or select **entire file**. [Gerrit review UI](https://gerrit-review.googlesource.com/Documentation/user-review-ui.html) | The change screen has one file list. Selecting a file opens its diff. [Gerrit review UI](https://gerrit-review.googlesource.com/Documentation/user-review-ui.html) |

## Assessment for Revisor

These tools keep the diff as the main place for change marks. They add unchanged source lines to that surface, or open a full file when the reviewer asks for it. A second full source view beside the diff makes the reviewer track two line positions and two sets of change marks. It also makes each code column narrower. This is a design inference from the layouts above, not a user study.

The changed-file list is navigation. GitHub and GitLab put it in one file tree or browser. A second list in the source panel repeats that job and takes space from the source. A related **test** can still be useful when it is a direct action near the selected file; a broad related-file list needs evidence that it finds files the normal file browser does not make easy to find. This is a product recommendation, not a feature of the cited tools.

## Recommended direction

1. Use the diff's existing context controls for nearby unchanged lines. Keep old and new versions in the diff view.
2. If reviewers need the full file, offer one **Open full file** action from the file header or a diff line. Show source in a focused view, with a clear way back to the same review position.
3. Keep changed-file navigation in the existing file list and search. Remove the repeated change marks and related changed-file list from the source context view. Keep a direct link to a matched test only if the match is useful and reliable.
4. Test with a long file and a multi-file PR: can a reviewer answer a context question, return to the exact diff line, and find a relevant test without losing review position?
