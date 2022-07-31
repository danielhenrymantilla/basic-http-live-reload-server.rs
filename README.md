# `http-live-reload-server`

```console
http-live-reload-server 0.8.1
Brian Anderson <andersrb@gmail.com>, Daniel Henry-Mantilla <danielhenrymantilla@gmail.com>
A basic HTTP file server, with live reload capabilities!

USAGE:
    http-live-reload-server [FLAGS] [OPTIONS] [ROOT]

FLAGS:
    -h, --help       Prints help information
    -V, --version    Prints version information
        --watch      Whether to spin a `watchexec` instance with the magical invocation

OPTIONS:
    -a, --addr <ADDR>       The IP:PORT combination. [default: 0.0.0.0:4000]
        --ws-port <PORT>    The port to use for the websocket server (for the live-reload feature) [default: 8090]

ARGS:
    <ROOT>    The root directory for serving files. [default: .]
```

___

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
    watchexec -e html,css,js -c --on-busy-update restart -- http-live-reload-server # <extra flags or args>
    ```

    Since this is expected to be so pervasive, `http-live-reload-server` comes
    with a flag to alias this:

    ```bash
    http-live-reload-server --watch # <extra flags or args>
    ```

      - Note that this will run whatever `watchexec` is in `PATH`!

[`watchexec`]: https://watchexec.github.io/

To increase logging verbosity use `RUST_LOG`:

```sh
RUST_LOG=http_live_reload_server=debug http-live-reload-server # <extra flags or args>
```

## License

MIT/Apache-2.0
