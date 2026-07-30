# herdr-window-title

Configurable outer-terminal title for [herdr](https://herdr.dev): template-driven,
session-aware, with a live agent-state indicator (blocked / done / working).

Default title: `herdr:<session>` — e.g. `herdr:personal`. When agents are active
the indicator takes the front: `●2 herdr:personal` (2 agents waiting for input),
`✓1 herdr:personal` (1 finished awaiting review), an animated spinner while the
focused agent works, or `②` for background work.

Full documentation (template syntax, config reference) lands with the first
public release. Requires herdr **0.7.4+**, macOS or Linux.

## Install

```sh
herdr plugin install mackt/herdr-window-title
```

Installation downloads a prebuilt, checksum-verified binary — no Rust toolchain
required.

## Development

`herdr plugin link` never runs build commands, so build first, then link:

```sh
git clone https://github.com/mackt/herdr-window-title
cd herdr-window-title
cargo build --release
herdr plugin link .
```

Run the tests (no herdr required — they speak to a fake herdr socket):

```sh
cargo test
```

Troubleshooting: config/template warnings appear in `herdr plugin log
--plugin mackt.window-title`; the background monitor's own warnings and API
failures append to `monitor.log` in the plugin state directory.

## License

MIT
