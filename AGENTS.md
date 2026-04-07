## Global Rules

- No broad searches in ~/, /Users/robertsale, ~/Library, or /Users/robertsale/Library (e.g. find, ripgrep, grep). Instead take the following approach:
  - `ls ~/`, then run targetted find or (rip)grep commands in the folder you need
  - Ask where to find the thing you are searching for may be located
- All skill scripts are in PATH. You do not need to use the absolute path to any skill script. Prefer executing skill scripts by basename.