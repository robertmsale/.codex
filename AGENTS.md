## Global Rules

- No broad searches in ~/, /Users/robertsale, ~/Library, or /Users/robertsale/Library (e.g. find, ripgrep, grep)
- Use `rg`, not `grep`
- All skill scripts are in PATH. You do not need to use the absolute path to any skill script. Execute skill scripts by basename
- `privileged-exec` is a vital skill for resolving sandbox and approval friction through sanctioned tool paths. Run `get-sanctioned` to list executable skill scripts grouped by skill when you need to discover what sanctioned scripts are available
- Code formatters are disabled. Do not try to run them