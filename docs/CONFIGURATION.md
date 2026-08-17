# Configuring OpenBiz

OpenBiz starts with no configuration at all. Run the binary and it binds `127.0.0.1:8080` and keeps
its data in `./data`. Everything below is optional.

There are three layers, lowest precedence first:

1. **Built-in defaults**
2. **A TOML configuration file**
3. **Environment variables**

The environment wins because it is what a container runtime, a systemd unit, or a one-off
`OPENBIZ_BIND=0.0.0.0:9000 ./openbiz` can reach without editing a file on disk.

## The configuration file

By default OpenBiz reads `openbiz.toml` from its working directory. **If that file is not there,
that is not an error** — the defaults apply.

Set `OPENBIZ_CONFIG` to read a file from somewhere else:

```sh
OPENBIZ_CONFIG=/etc/openbiz/openbiz.toml ./openbiz
```

If `OPENBIZ_CONFIG` names a file that does not exist, OpenBiz **stops with an error** rather than
falling back to the defaults. An explicit request that silently degrades is how a production server
comes up on the wrong port and nobody notices for a week.

### Every setting

```toml
# The address to bind. Default: "127.0.0.1:8080".
# Loopback is the default deliberately — a self-hosted server should not appear on a public
# interface because somebody unzipped it. Set this to "0.0.0.0:8080" to listen everywhere.
bind = "127.0.0.1:8080"

# Directory holding the RDF store and backups. Default: "./data".
data_dir = "./data"
```

Both keys are optional. A file that sets only `bind` leaves `data_dir` at its default.

| File key   | Environment variable | Default            |
|------------|----------------------|--------------------|
| `bind`     | `OPENBIZ_BIND`       | `127.0.0.1:8080`   |
| `data_dir` | `OPENBIZ_DATA_DIR`   | `./data`           |

Two further environment variables have no file equivalent, because both need to take effect before
any file is read:

| Variable         | Effect                                                                |
|------------------|-----------------------------------------------------------------------|
| `OPENBIZ_CONFIG` | Path to the configuration file. Default: `openbiz.toml` in the CWD.   |
| `OPENBIZ_LOG`    | Log filter, `tracing-subscriber` `EnvFilter` syntax. Default: `info`.  |

## Where did this value come from?

Most of the pain of configuring a governance platform is not writing the configuration — it is
working out which of several files, environment variables, and defaults actually won. OpenBiz
answers that at startup, per setting:

```
INFO openbiz: configuration setting="bind" value=0.0.0.0:9000 source=/etc/openbiz/openbiz.toml
INFO openbiz: configuration setting="data_dir" value=/srv/openbiz source=$OPENBIZ_DATA_DIR
```

`source` is one of the literal path of the file that supplied it, the `$NAME` of the variable that
supplied it, or `the built-in default`.

The same provenance appears when a setting turns out to be wrong. Binding a privileged port fails
with the place to go and edit, not just the value that did not work:

```
Error: failed to bind 0.0.0.0:80, from /etc/openbiz/openbiz.toml

Caused by:
    Permission denied (os error 13)
```

## Mistakes OpenBiz refuses to make quietly

**An unrecognised key is an error.** A key we do not know is almost always a typo, and the worst
possible response is to ignore it — the operator then believes they configured something they did
not. OpenBiz names the line, the key, and what it accepts instead:

```
Error: failed to load configuration

Caused by:
    0: openbiz.toml is not a valid OpenBiz configuration file
    1: TOML parse error at line 1, column 1
         |
       1 | bnd = "0.0.0.0:9000"
         | ^^^
       unknown field `bnd`, expected `bind` or `data_dir`
```

**A blank value is an error.** `OPENBIZ_BIND=` and `bind = ""` are both rejected, naming the source:

```
Error: failed to load configuration

Caused by:
    OPENBIZ_BIND is set to a blank value, from $OPENBIZ_BIND; remove it to use the default instead
```

An unset shell variable, a `docker compose` interpolation with nothing behind it, and a systemd
`Environment=` line all collapse to empty, so treating empty as "unset" would be a silent ignore
wearing a different hat. Remove the setting to get the default; do not blank it.

## What is not validated yet

`bind` is checked for being non-blank, not for being a resolvable `host:port` — a bad address is
caught when the server tries to bind it, which produces a clear error but does so slightly later
than it could. `data_dir` is carried and logged but **no code creates or opens that directory**; it
becomes real when Phase 1 wires the store. See `docs/UNTESTED.md`.
