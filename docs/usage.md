# Usage

Start Terminaltype with `./run`, or use `monkeytype-tui` after installing it with Cargo. Run `./run --help` for the complete option list.

## Test modes

| Mode | Behavior | Example |
| --- | --- | --- |
| Time | Ends at the selected duration | `./run --time 60` |
| Words | Ends when you finish the word list | `./run --words 50` |
| Quote | Uses an English quote | `./run --mode quote` |
| Zen | Accepts free typing until you press Enter | `./run --mode zen` |
| Custom | Uses text supplied by you | `./run --custom 'Practice a sentence.'` |

The first typed character starts the clock. Leading spaces do not start a test. Time limits range from 1 to 3,600 seconds; word limits range from 1 to 10,000 words. Quote length is available in the test controls.

Press `Esc` for the command palette. Type part of a command, move with the arrow keys, and press Enter. Theme and language commands open their own lists. Mouse controls work in the header, mode bar, dialogs, and footer.

## Corrections and difficulty

Backspace corrects one character. `Ctrl+W` or `Ctrl+Backspace` clears the current word. By default, you can return to a previous word if it contains an error; freedom mode lets you return to correct words too.

Normal difficulty lets you continue through mistakes. Expert difficulty ends the test when you submit an incorrect word. Master difficulty ends it at the first incorrect character. Blind mode hides error colors while you type; it does not remove mistakes from the result.

Paste is disabled in typing tests. The custom text dialog accepts paste so you can supply a passage before starting.

## Dictionaries and appearance

The dictionaries are `english`, `english_1k`, `english_5k`, `spanish`, `french`, `german`, `italian`, `portuguese`, `code_python`, `code_javascript`, and `code_rust`.

```sh
./run --language english_1k --words 100
./run --theme catppuccin
./run --scale 1.5 --fps 144
```

The default theme is `serika_dark`. Settings include font size, caret style, live WPM, blind mode, freedom mode, and difficulty. Font size changes the typing text; `--scale` changes the whole interface. Open settings and scroll for rows that do not fit in the window.

## Local data

The default directory is `$XDG_DATA_HOME/monkeytype-tui`, falling back to `~/.local/share/monkeytype-tui`.

| File | Contents |
| --- | --- |
| `config.json` | Settings selected in the app |
| `history.json` | Up to 1,000 test results |

Writes use a temporary file followed by a rename. A missing or unreadable file falls back to defaults. Command-line options override the loaded settings for that launch; settings changes made in the app are saved.

```sh
./run --data-dir /tmp/terminaltype-practice
```

Use one process per data directory. Concurrent processes do not coordinate their writes. The app does not upload your results.

## Terminal problems

If graphics are missing, check that you are running directly in Kitty and using the release build. Enlarge the terminal if the interface asks for more room. A smaller `--scale`, such as `0.8`, can help in a compact window.

For slow updates, compare the PTY and real Kitty tests in [rendering.md](rendering.md). They measure different parts of the path. A low CPU render time does not establish low display latency.

The app restores raw mode, mouse capture, the cursor, and the alternate screen on normal exit and termination signals. If a forced kill leaves your shell in raw mode, run `stty sane` and then `reset`.
