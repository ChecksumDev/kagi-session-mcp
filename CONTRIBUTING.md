# Contributing

Thanks for being here. This is a small project, so contributing should feel low-friction. The notes below are short on purpose.

## Reporting issues

Please include:

- Your OS and browser (and version, if you can grab it).
- A clipped log run with `RUST_LOG=kagi_session_mcp=debug` if anything actually broke. Logs land on stderr.
- The output of the `kagi_status` tool. That tells us whether discovery succeeded and which auth path it took.

If your report is "Kagi changed something and now `kagi_search` returns nothing," that's also fine, just say so. See the Kagi-changed-something section below.

## Pull requests

- Fork, branch off `main`, open the PR against `main`.
- Run `cargo fmt --all` and `cargo clippy --all-targets -- -D warnings` before pushing. CI checks both.
- Keep changes focused. A PR that fixes one thing lands quickly; a PR that fixes one thing and refactors three files lands slowly.
- Tests are nice to have but not required for small fixes. The CI matrix covers Linux, macOS, and Windows builds.

## Commit messages

We follow [Conventional Commits](https://www.conventionalcommits.org/). The release-please workflow uses them to cut versions, so this matters.

Common types:

- `feat:` user-visible new capability
- `fix:` bug fix
- `chore:` anything that isn't user-visible (tooling, deps, refactors with no behavior change)
- `docs:` README, comments, etc.
- `ci:` workflow changes

Optional scope in parentheses, e.g. `feat(search): add region filter` or `fix(chromium): handle Profile dirs without numbers`.

Breaking changes get a `!` after the type and a `BREAKING CHANGE:` footer.

## When Kagi changes its markup or routes

This is the most common kind of patch. Because we scrape Kagi's HTML and SSE protocol, we break whenever Kagi ships a redesign or renames a CSS class. The fix usually lives in one of these spots:

- `src/adapters/serp_parser.rs`: CSS selectors for the SERP, knowledge panel, lenses, vertical results.
- `src/adapters/kagi_http.rs`: SSE tag names (`search`, `wikipedia`, `related_searches`, etc.) and pagination param names.
- `src/domain/model.rs` and `src/mcp/schema.rs`: only if the *shape* of the data changed.

If you're chasing a parser break, the fastest loop is:

1. Run a real query in your browser, view-source the response, and grab the new class names or SSE tags.
2. Update the matching `Selector::parse(...)` constants or tag match arms.
3. Re-run with `RUST_LOG=kagi_session_mcp=debug` and confirm the parsed counts look sane.

When in doubt, open an issue with the diff before sending the PR. Some changes are subtle (Kagi sometimes A/B tests selectors), and a quick check helps avoid landing a fix that only works for half the users.

## Adding a new browser

Each browser source implements `SessionSource` (`src/domain/ports.rs`). For Chromium-family forks, add a `BrowserKind` variant and a path entry in `src/adapters/browser/chromium.rs`. For Firefox forks, add a `(label, profile_roots)` tuple in `src/adapters/browser/firefox.rs`. The discovery layer picks them up automatically.

## Code style

- Default to no comments; only add one when the *why* is non-obvious.
- Lowercase log messages.
- Maintain the hexagonal architecture.
- Prefer editing existing files over creating new ones.

## Releasing

Releases go through release-please and crates.io. Maintainers handle that; contributors don't need to bump versions.
