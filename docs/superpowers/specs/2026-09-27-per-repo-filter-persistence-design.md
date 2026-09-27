# Per-Repository Pull Request Filter Persistence

## Goal
Restore advanced pull request filters when the user opens the same repository again, including after an app restart.

## Design
- Save only the advanced filter selections: authors, labels, assignees, and review status.
- Save each repository's selections separately in local storage. Keep the current Open/Closed/All selection and text search behavior unchanged.
- Load a repository's saved selections before the first pull request query. The existing preset actions use the same filter state, so preset selections persist without separate preset state.
- Keep Clear filters as the way to clear and save an empty set of advanced filters. Continue to reset transient option lists when the list route closes; do not erase persisted filters during that reset.
- Treat missing, invalid, or unavailable local storage data as an empty filter set. Do not prevent the pull request list from opening if storage read or write fails.

## Verification
- Unit-test per-repository save and restore, independent repository values, empty values, and malformed stored values.
- Run the existing filter preset tests, TypeScript check, and production build.
- Check that repository filters restore before the initial query and that Clear filters remains clear after reopening.
