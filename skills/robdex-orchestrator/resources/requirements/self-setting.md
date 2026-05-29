# Setting Requirements On Self

Use this only when the operator explicitly asks you to set Requirements on your own current thread.

The only self-setting path is `--to-self`. Do not attach Requirements to yourself by omitting a target; the CLI intentionally rejects that because self-setting must restart the turn under the new schema.

From prose:

```bash
robdex requirements-from-prose --title "<title>" --include-composable no-legacy --text-stdin --to-self <<'EOF'
- Requirement one.
- Requirement two.
EOF
```

From a JSON file:

```bash
robdex set-requirements --to-self --requirements-file /absolute/path/to/requirements.json
```

The self path sets Requirements, briefly delays, interrupts the current thread, then sends `Begin` to start the next turn under the Requirements schema.

Never run `robdex requirements-from-prose ... --text-stdin` without a heredoc, pipe, or redirected file attached.
