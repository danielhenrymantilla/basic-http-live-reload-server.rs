# `http-live-reload-server`

Toy project. A fork of `basic-http-server` with:

  - dependencies updated;
  - a focus on serving actual static websites
    (_e.g._, no `-x` developer extensions)
  - with **live reload** as its main feature
  - everything else has been trimmed out to keep it simple (but there are
    still too many deps for my taste…).

## Installation and Use

**Note that `http-live-reload-server` is not production-ready and should not be
exposed to the internet. It is a learning and development tool.**

 1. Install with `cargo install`:

    ```bash
    # Notice the current lack of versioning: UNSTABLE!
    cargo install --git 'https://github.com/danielhenrymantilla/http-live-reload-server.rs'
    ```

 1. You can then run it with just:

    ```bash
    http-live-reload-server
    ```

 1. The recommended way of using it, for its live-reload capabilities, is to
    use it in conjunction with the excellent [`watchexec`] tool:

    ```bash
    watchexec -e html,css,js -c --on-busy-update restart -- http-live-reload-server # <extra flags>
    ```

[`watchexec`]: https://watchexec.github.io/

To increase logging verbosity use `RUST_LOG`:

```sh
RUST_LOG=http_live_reload_server=debug http-live-reload-server # <extra flags>
```

Command line arguments:

```
USAGE:
    http-live-reload-server [FLAGS] [OPTIONS] [ARGS]

FLAGS:
    -h, --help       Prints help information
    -V, --version    Prints version information

OPTIONS:
    -a, --addr <ADDR>    Sets the IP:PORT combination (default "0.0.0.0:4000")

ARGS:
    ROOT    Sets the root directory (default ".")
```

## License

MIT/Apache-2.0
