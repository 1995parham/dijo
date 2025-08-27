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
- **`dijo` is modal**: different modes to view different
  stats!
- **Vim like command mode**: add with `:add`, delete with
  `:delete` and above all, quit with `:q`!.
- **Fully scriptable**

### Install

To get the latest release of `dijo`, prefer installing it
via `cargo`. Unofficial packages exist for some package
managers as well. You can also browse the
[Releases](https://github.com/NerdyPepper/dijo/releases)
page for prebuilt binaries.

#### Cargo

```shell
# dijo requires rustc >= v1.42
$ rustup update

$ cargo install dijo
```

### Usage

`dijo` has a [detailed
wiki](https://github.com/NerdyPepper/dijo/wiki/), here are
some good places to start out:

- [Getting started](https://github.com/NerdyPepper/dijo/wiki/Getting-Started)
- [Automatically tracking habits](https://github.com/NerdyPepper/dijo/wiki/Auto-Habits)
- [Command reference](https://github.com/NerdyPepper/dijo/wiki/Commands)

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
