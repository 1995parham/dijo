<p align="center">
  <img height="200" src="./.github/assests/lz.png">
</p>

### About

`dijo` is a habit tracker. It is curses-based, it runs in
your terminal. `dijo` is scriptable, hook it up with external programs
to track events without moving a finger. `dijo` is modal,
much like a certain text editor.

### Features

- **Vim like motions**: navigate `dijo` with `hjkl`!
- **`dijo` is modal**: cycle a habit through `DAY`, `WEEK`,
  `MONTH`, `YEAR`, `STATS` and `HEATMAP` views with `v`!
- **Contribution heatmap**: a GitHub-style grid of your
  streaks, right in the terminal.
- **Full-screen dashboard**: press `d` for an at-a-glance
  summary of the focused habit — all-time stats and a
  year-long heatmap.
- **Vim like command mode**: add with `:add`, delete with
  `:delete` and above all, quit with `:q`!.
- **Fully scriptable**

### Install

To get the latest release of `dijo`, prefer installing it
via `cargo`. You can also browse the
[Releases](https://github.com/1995parham/dijo/releases)
page for prebuilt binaries (Linux, macOS and Windows).

#### Cargo

```shell
# dijo is built with the Rust 2024 edition; use a recent stable toolchain
$ rustup update

$ cargo install dijo
```

### Usage

Run `dijo` to open the grid of habits. The bundled man page
([`dijo.1`](./dijo.1)) covers everything in detail; here is the
short version.

#### Views

Press `v` to cycle the view for every habit; `Esc` returns to
`DAY` and resets the cursor.

| View      | Shows                                                       |
| --------- | ---------------------------------------------------------- |
| `DAY`     | every day of the month as a grid                           |
| `WEEK`    | weekly completion bars                                      |
| `MONTH`   | per-month completion for the year                          |
| `YEAR`    | per-year completion bars                                    |
| `STATS`   | current/longest streak, total completions, completion rate |
| `HEATMAP` | a contribution grid of trailing weeks (`[` `]` to scroll)  |

#### Keybindings

| Key             | Action                                  |
| --------------- | --------------------------------------- |
| `h` `j` `k` `l` | move focus between habits               |
| `H` `J` `K` `L` | move the day cursor                     |
| `n` / `Enter`   | increment today (`+1`)                  |
| `p` / `Backspace` | decrement today (`-1`)                |
| `v`             | cycle the view mode                     |
| `d`             | open the focused habit's dashboard      |
| `[` `]`         | sift to the previous / next month       |
| `Esc`           | reset view and cursor                   |
| `:`             | enter command mode                      |

#### Commands

`:add <name> [goal]`, `:delete <name>`, `:month-prev` / `:mprev`,
`:month-next` / `:mnext`, `:archive`, `:dashboard` / `:dash`,
`:write` / `:w`, `:quit` / `:q`, `:writeandquit` / `:wq`,
`:help [<command>|commands|keys]`.

## Design Notes

habit:

```
type: bit/count
stats:
  year:
    month:
      bit:
      |-- dates - array
      count:
      |-- dates - k,v pairs
```

habit:
-type: `bit/count`
-stats: `k,v (dates, bit/count)`

Cycle habit type:

- `n` states
- cycles through states on `prev` `next` events
- represent by symbol/char
- `ser` to `usize`?

Modes:

- Day mode - shows all days of 1 month
  - shift months on Previous/Next
- Week mode?
  - Aggregate stats for 1 week
  - show 4 weeks per view
  - bar graph for count and bit

Command mode:

- Add command
  - `add <name> <type> <goal>`
  - `add <name> --type <type> [--goal <goal>]`
  - Interactive add command via questionnaire?
- Edit command?
  - `edit <name> <new-type> <new-goal>`
  - `edit <name> --goal <new-goal>`
  - `edit <name> --type <new-type>`
  - Interactive edit command via questionnaire?
- Delete command
  - `delete <name>`
  - `delete _ (deletes focused?)`
- Chronological nav:
  - `month-prev` `mprev`
  - `month-next` `mnext`

Interface:

- Move view port if focused view goes outside bounds
- Tab completion for command mode? Requires Lex table
- Move command window to bottom, styling
- Prefix command window with `:`

Undo-tree:

- Store app states in memory
- Should store diffs? Or entire state?
- Ideal undo depth limit?
