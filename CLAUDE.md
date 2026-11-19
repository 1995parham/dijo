# CLAUDE.md

Guidance for working in this repo: **dijo**, a scriptable, curses-based, modal
habit tracker (the `1995parham` fork). Rust 2024 edition, `cursive` TUI.

## Build, lint, test

CI (`.github/workflows/ci.yml`) runs on every push and is the gate. Run all
three locally **before committing or pushing** — formatting is enforced and is
the easiest check to trip:

```sh
cargo fmt --all -- --check    # CI fails if code isn't rustfmt'd (use `cargo fmt` to fix)
cargo clippy --all-targets -- -D warnings   # warnings are hard errors in CI
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
- `src/views.rs` — `ShadowView`: the body of a habit card for each view mode
  (day / week / month / sparkline / year / stats / heatmap). The card frame,
  title and progress badge are drawn by the grid (`draw_card` in
  `impl_view.rs`), which passes each habit its accent color.
- `src/theme.rs` — Cursive theme plus color helpers: `displayable` (RGB →
  nearest base color without truecolor) and `progress_color` (shades of an
  accent for partial progress).
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

- **Card grid** (`impl_view.rs`): each habit is a rounded card, `CARD_WIDTH` ×
  `VIEW_HEIGHT`, coloured with `CONFIGURATION.accent(idx)` (the `accents` list
  in `config.toml`, cycled by grid position). The column count (`App::cols`) is
  set in `required_size` from the terminal width, 1 to `GRID_WIDTH`.
- **Visual check**: run the binary in tmux with `HOME` pointed at a scratch
  directory (never the real data dir) and read `tmux capture-pane -p`.

- View modes cycle with `v`: Day → Week → Month → Sparkline → Year → Stats →
  Heatmap; `Esc` resets to Day. The mode applies to all habits at once.
- **Sparkline** (`views.rs`): a compact one-row month view — each day is a bar
  `▁`..`█` whose height tracks that day's completion ratio (`(goal-remaining)/goal`),
  coloured reached/partial/missed; future days are blank, the cursor day is
  highlighted.
- **Weekly goals**: a habit's goal carries a `GoalPeriod` (`Daily`/`Weekly`,
  `prelude.rs`), stored per habit with `#[serde(default)]` so old records load as
  daily. `:add gym 3/week` (also `/w`, `/weekly`) makes a `Count`/`Float` whose
  `reached_goal`/`remaining` aggregate the Mon–Sun week (`week_bounds` in
  `utils.rs`). A weekly `1` stays a `Count(1)` ("once a week"), not a `Bit`; `Bit`
  is daily-only. `habit_stats` is period-aware: it reduces reached dates to
  per-period indices, so a `Weekly` habit's streak/total/rate count *weeks*, and
  the Stats view and dashboard label the unit accordingly.
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
- `command.rs` now has parse tests (incl. weekly goals); still no tests for
  `archive_habits` month-grouping or `missed_dates`.
- The archive scanner reduces to the pure, tested `utils::archived_reached_dates`
  and is period-aware. One residual approximation: it aggregates weekly goals
  per archive *file* (i.e. per month), so a Mon–Sun week straddling a month
  boundary is summed on each side separately. Acceptable given per-month archive
  granularity.
- `:archive` merges into an existing archive file (`utils::merge_archive`) and
  writes atomically (`utils::write_atomic`); the live record is trimmed only
  after every file is written.
- The habit model is still trait objects (`typetag`); an enum with
  `#[serde(tag = "type")]` would read the same JSON and remove the
  `HabitWrapper`/`ShadowView` traits and macros. View state (`InnerData`) is
  duplicated per habit and should move to `App`.
