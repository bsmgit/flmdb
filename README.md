# flmdb

**Film Database CLI** — a fast, terminal-first cataloging tool for physical and digital movie collections.

`flmdb` tracks film media, standardizes unique IMDb identifiers, and hydrates rich metadata directly from the [OMDb API](https://www.omdbapi.com/). Everything is stored in a single local SQLite file — no server, no account, no cloud.

- **Version:** 1.0.0
- **Language:** Rust (edition 2021)
- **Storage:** SQLite (`flmdb.db`, created automatically)
- **License:** MIT

---

## Table of Contents

- [Features](#features)
- [Requirements](#requirements)
- [Installation](#installation)
- [Quick Start](#quick-start)
- [Configuration](#configuration)
- [Command Reference](#command-reference)
  - [`add`](#add)
  - [`list`](#list)
  - [`export`](#export)
  - [`import`](#import)
  - [`hydrate`](#hydrate)
  - [`api-key`](#api-key)
- [CSV Format](#csv-format)
- [Data Model](#data-model)
- [Behavior Notes & Gotchas](#behavior-notes--gotchas)
- [Troubleshooting](#troubleshooting)
- [Uninstalling](#uninstalling)
- [License](#license)

---

## Features

- Catalog films by title, year, and media format (DVD, Blu-ray, 4K, VHS, digital — anything you type).
- Store the canonical IMDb ID (`tt0133093`) as a unique key per film.
- Automatically pull director, genre, runtime, and plot from OMDb.
- Full-text-ish search across titles.
- CSV import and export for bulk edits, backups, and migration.
- API key stored in the database, so it survives across shells and sessions.
- Single self-contained binary; SQLite is compiled in (no system SQLite needed).

---

## Requirements

### Runtime

Just the compiled binary. SQLite is statically bundled.

### Build-time

| Requirement | Notes |
|---|---|
| Rust toolchain | 1.70 or newer (1.74+ recommended). Install via [rustup](https://rustup.rs/). |
| C compiler | Needed to build the bundled SQLite (`gcc`, `clang`, or MSVC). |
| OpenSSL + `pkg-config` | Linux only — `reqwest` uses the system TLS stack by default. |

Platform-specific setup:

```bash
# Debian / Ubuntu
sudo apt update
sudo apt install build-essential pkg-config libssl-dev

# Fedora / RHEL
sudo dnf install gcc pkgconf-pkg-config openssl-devel

# macOS (Xcode Command Line Tools provide clang; TLS uses Secure Transport)
xcode-select --install

# Windows
# Install the "Desktop development with C++" workload from Visual Studio Build Tools,
# then use the default x86_64-pc-windows-msvc Rust toolchain.
```

### Optional

An **OMDb API key** — free tier available at <https://www.omdbapi.com/apikey.aspx>. Only required for the `hydrate` command. All other commands work fully offline.

---

## Installation

### From source

```bash
git clone <your-repository-url> flmdb
cd flmdb
cargo build --release
```

The binary lands at `target/release/flmdb` (`target\release\flmdb.exe` on Windows).

### Install to your PATH

```bash
# From inside the project directory
cargo install --path .
```

This places `flmdb` in `~/.cargo/bin`. Make sure that directory is on your `PATH`:

```bash
# bash / zsh — add to ~/.bashrc or ~/.zshrc
export PATH="$HOME/.cargo/bin:$PATH"
```

### Manual copy

```bash
sudo install -m 755 target/release/flmdb /usr/local/bin/flmdb
```

### Verify

```bash
flmdb --version
flmdb --help
```

---

## Quick Start

```bash
# 1. Add a film manually
flmdb add --title "The Matrix" --year 1999 --format "4K" --imdb tt0133093

# 2. Save your OMDb API key (one time)
flmdb api-key set abcd1234

# 3. Pull down director, genre, runtime, and plot
flmdb hydrate

# 4. Look at what you have
flmdb list --verbose

# 5. Back it up
flmdb export --file my-collection.csv
```

---

## Configuration

### Database location

`flmdb` opens (and creates, if missing) a file named **`flmdb.db` in the current working directory**. There is no global or home-directory default.

This means the catalog you see depends on where you run the command from. Two common approaches:

**Pick a home for your collection and always run from there:**

```bash
mkdir -p ~/films && cd ~/films
flmdb add --title "Alien" --year 1979 --format "Blu-ray"
```

**Or add a shell alias that always uses one directory:**

```bash
# ~/.bashrc or ~/.zshrc
alias flmdb='(cd ~/films && command flmdb "$@")'
```

SQLite runs in WAL mode, so you'll also see `flmdb.db-wal` and `flmdb.db-shm` alongside the main file. Those are normal — keep them with the database when moving it, or run any SQLite client checkpoint before copying.

### API key resolution

`hydrate` looks for the OMDb key in this order:

1. The `omdb_api_key` row in the database's `config` table (set via `flmdb api-key set`).
2. The `OMDB_API_KEY` environment variable.

If neither is present, `hydrate` prints an error and exits without making any network calls.

```bash
# Environment variable alternative — useful for CI or throwaway shells
export OMDB_API_KEY="abcd1234"
flmdb hydrate
```

---

## Command Reference

Global flags:

| Flag | Description |
|---|---|
| `-h`, `--help` | Show help. Works on every subcommand too. |
| `-V`, `--version` | Print the version. |

---

### `add`

Add a new film to the catalog manually. Does not contact the network.

```
flmdb add --title <TITLE> --year <YEAR> [--format <FORMAT>] [--imdb <IMDB_ID>]
```

| Flag | Short | Type | Default | Required |
|---|---|---|---|---|
| `--title` | `-t` | string | — | yes |
| `--year` | `-y` | integer | — | yes |
| `--format` | `-f` | string | `DVD` | no |
| `--imdb` | `-i` | string | none | no |

Examples:

```bash
flmdb add --title "Blade Runner" --year 1982
flmdb add -t "Arrival" -y 2016 -f "Blu-ray" -i tt2543164
flmdb add --title "Nosferatu" --year 1922 --format "Digital"
```

Output:

```
✅ Added: "Arrival" (2016) [Blu-ray] (IMDb ID: tt2543164)
```

Supplying the IMDb ID at add time is optional but recommended — it's the only thing `hydrate` can use to look a film up later.

---

### `list`

Print the catalog, sorted alphabetically by title.

```
flmdb list [--search <QUERY>] [--verbose]
```

| Flag | Short | Description |
|---|---|---|
| `--search` | `-s` | Case-insensitive substring filter applied to the **title** only. |
| `--verbose` | `-v` | Add director, genre, and full plot summaries to the output. |

Examples:

```bash
flmdb list
flmdb list --search matrix
flmdb list -s "star" -v
```

Standard output:

```
ID    IMDb ID      Title                               Year   Format
----------------------------------------------------------------------
3     tt2543164    Arrival                             2016   Blu-ray
1     tt0133093    The Matrix                          1999   4K
----------------------------------------------------------------------
Total records displayed: 2
```

Verbose output adds `Director` and `Genre` columns plus an indented `Plot:` line beneath each record. Fields that haven't been hydrated show as `-`.

If nothing matches, you get `No matching films found.`

---

### `export`

Write the entire catalog to a CSV file, ordered by database ID.

```
flmdb export [--file <PATH>]
```

| Flag | Short | Default |
|---|---|---|
| `--file` | `-f` | `catalog.csv` |

```bash
flmdb export
flmdb export --file ~/backups/films-2026-07.csv
```

Output: `💾 Successfully exported 42 items to 'catalog.csv'`

An existing file at the destination is overwritten without prompting.

---

### `import`

Bulk-load films from a CSV file. Runs inside a single transaction — if any row fails, nothing is committed.

```
flmdb import --file <PATH>
```

| Flag | Short | Required |
|---|---|---|
| `--file` | `-f` | yes |

```bash
flmdb import --file starter-collection.csv
```

Output: `⚡ Successfully imported 42 films into flmdb from 'starter-collection.csv'`

See [CSV Format](#csv-format) for the expected columns.

---

### `hydrate`

Fill in missing metadata (director, genre, runtime, plot) from the OMDb API. Requires an API key.

```
flmdb hydrate [--imdb <IMDB_ID>]
```

| Flag | Short | Behavior |
|---|---|---|
| *(none)* | | Hydrates every record that has an IMDb ID **and** is missing a director or a plot. |
| `--imdb` | `-i` | Hydrates exactly one record, matched by its IMDb ID — regardless of whether it already has metadata. |

```bash
flmdb hydrate
flmdb hydrate --imdb tt0133093
```

Output:

```
🔍 Hydrating metadata for 3 entries...
  ✓ Hydrated: IMDb ID tt0133093
  ✓ Hydrated: IMDb ID tt2543164
  ✗ API Error for tt9999999: Incorrect IMDb ID.
🎉 Hydration complete.
```

Requests are throttled with a 150 ms pause between films to stay comfortably inside OMDb's free-tier rate limits. A batch of 100 films takes roughly 15 seconds of pure waiting, plus network time.

Per-film API failures are reported and skipped; the run continues. A transport-level failure (no network, DNS failure) aborts the whole command with an error.

---

### `api-key`

Manage the stored OMDb API key.

```
flmdb api-key set <KEY>
flmdb api-key list
flmdb api-key reset
```

| Subcommand | Description |
|---|---|
| `set <KEY>` | Save (or replace) the key in the database. Positional argument — no flag needed. |
| `list` | Display the currently saved key. |
| `reset` | Delete the saved key. |

```bash
flmdb api-key set abcd1234
# ✅ OMDb API key saved successfully.

flmdb api-key list
# 🔑 Current OMDb API Key: abcd1234

flmdb api-key reset
# 🗑️ OMDb API key has been reset/removed.
```

> **Security note:** the key is stored as plain text in `flmdb.db` and `api-key list` prints it in full. Treat the database file as a secret, keep it out of version control, and prefer the `OMDB_API_KEY` environment variable on shared machines.

---

## CSV Format

### Import

The importer reads these **named** headers. Column order doesn't matter; a header row is required.

| Column | Type | Required | Notes |
|---|---|---|---|
| `title` | text | yes | |
| `year` | integer | yes | Must parse as a whole number. |
| `format` | text | yes | Free-form. Cannot be empty — use `DVD` or `Unknown` as a filler. |
| `imdb_id` | text | no | May be blank. Must be unique across the catalog. |

Any additional columns are ignored, which means a file produced by `flmdb export` can be fed back into `flmdb import`.

Minimal example:

```csv
title,year,format,imdb_id
The Matrix,1999,4K,tt0133093
Blade Runner,1982,Blu-ray,tt0083658
Nosferatu,1922,Digital,
```

### Export

Exported files always carry all nine columns in this order:

```csv
id,imdb_id,title,year,format,director,genre,runtime,plot
1,tt0133093,The Matrix,1999,4K,"Lana Wachowski, Lilly Wachowski","Action, Sci-Fi",136 min,"A computer hacker learns..."
```

Empty optional fields are written as empty strings, not `NULL`.

---

## Data Model

Two tables, created and migrated automatically on every run.

### `movies`

| Column | Type | Constraints |
|---|---|---|
| `id` | INTEGER | PRIMARY KEY AUTOINCREMENT |
| `title` | TEXT | NOT NULL |
| `year` | INTEGER | NOT NULL |
| `format` | TEXT | NOT NULL |
| `imdb_id` | TEXT | UNIQUE, nullable |
| `director` | TEXT | nullable |
| `genre` | TEXT | nullable |
| `runtime` | TEXT | nullable |
| `plot` | TEXT | nullable |

Indexes: `idx_movies_title` on `title`, `idx_movies_imdb` on `imdb_id`.

### `config`

| Column | Type | Constraints |
|---|---|---|
| `key` | TEXT | PRIMARY KEY |
| `value` | TEXT | NOT NULL |

Currently holds a single row with key `omdb_api_key`.

Because the schema is created with `IF NOT EXISTS` and additive `ALTER TABLE` migrations, upgrading from an older database happens transparently — nothing is dropped or rewritten.

---

## Behavior Notes & Gotchas

- **The database is per-directory.** Running `flmdb list` from a different folder shows an empty (freshly created) catalog. See [Configuration](#configuration).
- **IMDb IDs are unique.** Adding or importing a second film with an IMDb ID already in the catalog fails with a constraint error. On import, the whole transaction rolls back, so nothing partial gets written.
- **Re-importing an export will collide.** Every row already has an IMDb ID, so a re-import into the same database hits the unique constraint. Import into a fresh directory, or strip `imdb_id` first.
- **Hydration overwrites.** For the fields it manages (`director`, `genre`, `runtime`, `plot`), hydration replaces whatever was there — including replacing a manually written value with a blank if OMDb has nothing. `title` and `year` are only overwritten when OMDb returns a value.
- **Only films with an IMDb ID can be hydrated.** Records without one are silently skipped by the bulk run. Use `flmdb add` again or edit the database directly to attach an ID.
- **Search is title-only.** `--search` does not look at director, genre, or plot.
- **Verbose columns can wrap.** Titles longer than 30 characters (35 in the compact view) push the row wider than the header rules. Use a wide terminal, or export to CSV for long titles.
- **Hydration uses plain HTTP.** Requests go to `http://www.omdbapi.com/`, so the API key travels unencrypted. Avoid running `hydrate` on untrusted networks.
- **Exit codes.** Success returns `0`. An unrecoverable error prints `Error: <message>` to stderr and returns a non-zero status. A missing API key is treated as a clean exit (`0`) with a message on stderr.

---

## Troubleshooting

**`error: linker 'cc' not found` during build**
Install a C compiler — see the [build requirements](#build-time). The bundled SQLite needs one.

**`Could not find directory of OpenSSL installation` / `openssl-sys` build failure**
On Linux, install `libssl-dev` (Debian/Ubuntu) or `openssl-devel` (Fedora/RHEL) plus `pkg-config`.

**`UNIQUE constraint failed: movies.imdb_id`**
That IMDb ID is already in the catalog. Check with `flmdb list --search "<title>"`.

**`❌ Error: No OMDb API key found.`**
Run `flmdb api-key set YOUR_KEY`, or export `OMDB_API_KEY` in your shell.

**`✗ API Error for ttXXXXXXX: Invalid API key!`**
The saved key is wrong or expired. Confirm with `flmdb api-key list`, then re-set it. Free OMDb keys require email activation before they work.

**`✗ API Error: Request limit reached!`**
The free tier caps daily requests. Hydrate in smaller batches across days, or upgrade the key.

**`✨ No records require hydration.`**
Everything with an IMDb ID already has a director and a plot. To force a refresh of one film, use `flmdb hydrate --imdb ttXXXXXXX`.

**`File 'x.csv' not found.`**
The path is relative to your current directory. Use an absolute path if unsure.

**`CSV deserialize error: missing field 'format'`**
The header row is missing a required column. See [CSV Format](#csv-format).

**Catalog looks empty after it worked yesterday**
You're probably in a different directory. `ls flmdb.db` to confirm, or `find ~ -name flmdb.db` to locate the real one.

---

## Uninstalling

```bash
# Remove the binary
cargo uninstall flmdb          # if installed via cargo install
sudo rm /usr/local/bin/flmdb   # if copied manually

# Remove your data (irreversible — export first if you want a backup)
rm flmdb.db flmdb.db-wal flmdb.db-shm
```

---

## License

MIT License

Copyright (c) 2026 flmdb contributors

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom it is furnished to do
so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.

---

### Third-party licenses

`flmdb` links against these crates, all under permissive licenses:

| Crate | Purpose | License |
|---|---|---|
| `clap` | CLI parsing | MIT / Apache-2.0 |
| `serde`, `serde_json` | Serialization | MIT / Apache-2.0 |
| `csv` | CSV read/write | Unlicense / MIT |
| `rusqlite` (bundled) | SQLite bindings | MIT |
| `reqwest` | HTTP client | MIT / Apache-2.0 |
| SQLite (bundled C source) | Storage engine | Public domain |

Film metadata retrieved through `hydrate` comes from the OMDb API and is subject to [OMDb's terms of use](https://www.omdbapi.com/legal.htm). It is not covered by this project's license.
