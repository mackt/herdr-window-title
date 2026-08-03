# herdr-window-title

Configurable outer-terminal title for [herdr](https://herdr.dev): template-driven,
session-aware, with a live agent-state indicator.

Your terminal tab stops saying `herdr` (or worse, `herdr session attach personal`)
and starts saying something useful:

```
herdr:personal            idle
⠙ herdr:personal          an agent is working
✓1 herdr:personal         an agent finished and awaits your review
●2 herdr:personal         two agents are waiting for YOUR input
herdr:personal (devbox)   the same session, attached over SSH
```

Works with any terminal that displays OSC window titles (Ghostty, WezTerm,
Kitty, iTerm2, tmux `set-titles`, …). Requires herdr **0.7.4+**, macOS or Linux.

## Install

```sh
herdr plugin install mackt/herdr-window-title
```

Installation downloads a prebuilt, SHA256-verified binary for your platform
(macOS/Linux × x86_64/arm64) — no Rust toolchain required.

**Using `herdr --remote`?** Install the plugin on the **remote server machine**
— event hooks and the title monitor run server-side, and the title reaches
your local terminal through the attach stream. Remote sessions name themselves
out of the box: the plugin detects the SSH environment and the default title
becomes `herdr:personal (devbox)`, while local sessions stay `herdr:personal`.

## How the title is built

The title is rendered from a template. The default is:

```
{indicator}herdr:{session}[ ({host})]
```

On a local server `{host}` is empty (see `host_display` below), so the
bracketed section collapses and the title is just `herdr:personal`; on a
server reached over SSH it becomes `herdr:personal (devbox)`.

### Tokens

| Token | Value |
|-------|-------|
| `{indicator}` | Agent-state indicator (see below); empty when idle. Carries a trailing space when non-empty so it collapses cleanly. |
| `{session}` | The herdr session name (`personal`, …); `default` for the unnamed session. |
| `{workspace}` | Focused workspace label. |
| `{tab}` | Focused tab's label, or its switch-order number (prefix+N) when the label is just a number. |
| `{agent}` | Detected agent in the focused pane (`claude`, `codex`, …). |
| `{title}` | The focused pane's terminal title (what the agent reports via OSC). |
| `{host}` | Short hostname of the machine the herdr server runs on. By default only rendered when that machine was reached over SSH — set `host_display = "always"` (or `"never"`) to override. |

Unknown tokens render literally (so a typo is visible in the title) and warn in
the plugin log.

### Optional sections

Wrap a part in `[ … ]` and it vanishes when every token inside is empty — no
dangling separators:

```toml
template = "{indicator}herdr:{session}[ · {workspace}][ ({host})]"
```

renders `herdr:personal · dotfiles (mbp)` in a workspace, `herdr:personal` when
nothing else resolves. Escape literal brackets/braces with `\`.

### The indicator

Exactly one segment, highest priority wins, counted across the whole session:

1. `●N` — N agents **blocked**, waiting for your input
2. `✓N` — N agents **done**, finished and not yet looked at
3. `⠋⠙⠹…` — animated spinner while an agent **in scope** works
4. `①`–`⑳`, `⊕` — count of working agents *outside* the scope
5. empty — nothing happening

Counts cap at `9+`. The scope defaults to the whole **session**: any working
agent anywhere spins, and the out-of-scope count never appears. Set
`spinner_scope = "pane"` to spin only for the focused pane (other work shows
as a count), or `"workspace"` for any pane in the focused workspace.

## Configuration

`config.toml` in the plugin config dir (`herdr plugin config-dir
mackt.window-title`). **Edits apply on the next refresh — no herdr restart.**
Every key is optional:

```toml
template = "{indicator}herdr:{session}[ ({host})]"

# Per-state overrides; each falls back to `template` when unset.
working_template = ""
blocked_template = ""     # e.g. "●{session} NEEDS YOU"
done_template = ""

# When {host} resolves to the server hostname: "auto" only when the server
# was reached over SSH (SSH_CONNECTION/SSH_CLIENT/SSH_TTY in its environment),
# "always", or "never". Empty {host} collapses its [ … ] section.
host_display = "auto"

spinner_scope = "session" # "session" | "workspace" | "pane"
spinner_interval_ms = 200 # spinner frame rate while in-scope work runs
idle_keepalive_ms = 2000  # title re-assert cadence (also the reattach fix)

blocked_glyph = "●"
done_glyph = "✓"
```

Invalid values fall back to their defaults with a warning in `herdr plugin log
--plugin mackt.window-title`; a broken template never loses the title.

## How it works

Event hooks are fire-and-forget: they ensure a single background **monitor**
process exists and poke it. The monitor is the only writer — it polls herdr's
public `session.snapshot` API, renders the template, and sets the title via
`client.window_title.set`. It re-asserts the title every `idle_keepalive_ms`,
which is what restores your title after detach/reattach (herdr stores no title
server-side). If the monitor dies, the next event hook takes over. When the
herdr server goes away, the monitor exits on its own.

The monitor's own warnings and API failures append to `monitor.log` in the
plugin state directory.

## Privacy

The plugin reads **only herdr's public socket API**. It never reads your agent
conversation files, so prompt text can never leak into a window title.

## Development

`herdr plugin link` never runs build commands, so build first, then link:

```sh
git clone https://github.com/mackt/herdr-window-title
cd herdr-window-title
cargo build --release
herdr plugin link .
```

Tests need no herdr install — they run the real binary against a fake herdr
socket:

```sh
cargo test
```

To watch the monitor's per-tick decisions against a live herdr, run it in the
foreground with `HWT_DEBUG=1` (kill the background monitor first):

```sh
pkill -f "herdr-window-title monitor"
HWT_DEBUG=1 HERDR_SOCKET_PATH=~/.config/herdr/sessions/<name>/herdr.sock \
  ./target/release/herdr-window-title monitor
```

## License

[MIT](LICENSE)
