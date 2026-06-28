# CLAUDE.md

Guidance for working in this repo: **dijo**, a scriptable, curses-based, modal
habit tracker (the `1995parham` fork). Rust 2024 edition, `cursive` TUI.

## Build, lint, test

CI (`.github/workflows/ci.yml`) runs on every push and is the gate. Run all
three locally **before committing or pushing** — formatting is enforced and is
the easiest check to trip:

```sh
cargo fmt --all -- --check    # CI fails if code isn't rustfmt'd (use `cargo fmt` to fix)
cargo clippy -- -D warnings   # warnings are hard errors in CI
cargo test
```

## Architecture

- `src/main.rs` — entry point: clap CLI (`--list`, `--missing`), builds the
  Cursive TUI, holds the `CONFIGURATION` static.
- `src/app/` — the `App` model (`Vec<Box<dyn HabitWrapper>>`, focus, cursor,
  message).
  - `impl_self.rs` — App logic: `load_state`/`save_state`, `archive_habits`,
    command dispatch (`parse_command`), and `focused_dashboard`.
  - `impl_view.rs` — Cursive `View` impl: grid drawing and key handling
    (`on_event`, including the `v` view cycle and the `d` dashboard hotkey).
- `src/habit/` — habit types `Bit`, `Count`, `Float` behind the `Habit` +
  `HabitWrapper` traits (typetag-serialized). `ViewMode` lives in `prelude.rs`.
- `src/views.rs` — `ShadowView`: per-habit rendering for each view mode
  (day / week / month / year / stats / heatmap).
- `src/stats.rs` — pure, unit-tested streak/total/rate math (`habit_stats`),
  reused by the Stats view and the dashboard.
- `src/utils.rs` — config + filesystem paths, archive scanning.
- `src/command.rs` — command parsing (`FromStr for Command`), the `:` command
  window, and `open_dashboard`.

Data lives in the platform data dir: `habit_record.json` plus an `archive/` of
`{month}_{year}.json` files (written by `:archive`). Config is `config.toml`.

## Conventions

- **Commits**: small and focused, one logical change each; conventional-commit
  prefixes (`feat:`, `fix:`, `refactor:`, `docs:`, `style:`, `chore:`).
  **Do not add Co-Authored-By / AI-attribution lines to commits.**
- **Error handling**: filesystem operations must not panic. The path helpers in
  `utils.rs`, plus `load_state`/`save_state`, return `Result<_, String>`;
  surface failures as a status-line message or a clean stderr exit. Config falls
  back to defaults on any problem. `save_state` writes atomically (temp file +
  rename) so a crash can't truncate the record.
- Infallible `unwrap`s are acceptable for date math, already-validated input, and
  in-memory serde of known-good structs — don't convert those to `Result`.
- **Tests**: prefer extracting pure logic (like `stats.rs`) and unit-testing it.

## Features (key behaviours)

- View modes cycle with `v`: Day → Week → Month → Year → Stats → Heatmap;
  `Esc` resets to Day. The mode applies to all habits at once.
- **Heatmap** (`views.rs`): GitHub-style grid shaded `█`/`▒`/`░` by completion,
  folding in archived reached-days; the rightmost column tracks the viewed week,
  so `[` / `]` scrolls it through history.
- **Dashboard** (`d`, or `:dashboard` / `:dash`): full-screen overlay for the
  focused habit (all-time stats + a labelled year-long heatmap). Built in
  `App::focused_dashboard`, shown via `command::open_dashboard`, dismissed with
  `q` or `Esc`.

## Release process

Pushing a tag `v*.*.*` triggers `.github/workflows/release.yml`: it
cross-compiles Linux / macOS (x86_64 + aarch64) / Windows, packages the binary
with `README.md` and `dijo.1`, and publishes a GitHub Release. Before tagging:
bump `version` in `Cargo.toml` (rebuild to sync `Cargo.lock`), update the `.TH`
line in `dijo.1`, and **confirm CI is green first**.

## Known gaps / TODO

- `GoalKind::Addiction` (`:add foo <5`) parses but falls through to a count-0
  habit in `parse_command` — implement an addiction/limit habit type or reject
  it with a clear message.
- No tests yet for command parsing (`command.rs`), `archive_habits`
  month-grouping, or `missed_dates`.
- Heatmap glyphs/colors are hardcoded; everything else is themeable via
  `config.toml`.
