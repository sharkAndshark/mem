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

## Docker

Use Docker if the host machine reports `GLIBC_x.y not found`.

Build the image:

```bash
docker build -t mem .
```

Run with custom targets:

```bash
docker run --rm -it mem -c 50% -m 60%
```

Stop with `Ctrl-C`.

If you download the Docker image from GitHub Releases instead of building locally:

```bash
docker load -i mem-docker-linux-amd64.tar.gz
docker run --rm -it mem -c 50% -m 60%
```
