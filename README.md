# mem

Dynamic CPU and memory stress tool for Linux and Windows.

## Usage

Only two options are supported:

- `-c, --cpu <PERCENT>`
- `-m, --memory <PERCENT>`

Both must be percentages in the range `0%..100%`.

Examples:

```bash
mem -c 50% -m 60%
mem -c 100% -m 90%
mem -c 0% -m 25%
```

The process runs continuously until `Ctrl-C`.

## Behavior

- CPU: dynamically drives total CPU load to target percentage.
- Memory:
  - Linux: controls using process RSS as observed memory.
  - Windows: controls using process Private Bytes / Commit memory.
- Status line is printed every 5 seconds.

## Build

```bash
cargo build --release
```
