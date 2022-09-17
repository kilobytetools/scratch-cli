# scratch-cli

This is the standalone cli for the scratch tool of [kilobyte tools](https://kilobytetools.io).  Scratch is a small
tool to move small amounts of short-lived data between two places with coarse security settings.

For example, to copy aws credentials to a machine which will be used to bootstrap the rest of the system, you might
run the following from your local machine:

```
local$ scratch push --burn < bootstrap/creds.txt
97a292d7
```

Then on the production machine being set up:

```
prod$ mkdir ~/.aws
prod$ scratch pull 97a292d7 > ~/.aws/credentials
```

Of course, you can copy your init script the same way:

```sh
local$ scratch push --no-private --lifetime 5m --url < bootstrap/init.sh
https://2e7e3b91.kilobytetools.io/scratch/file/128b1cc0


# using raw url
prod$ curl https://2e7e3b91.kilobytetools.io/scratch/file/128b1cc0 > init.sh
prod$ ./init.sh

# using latest pushed
backup$ scratch pull > init.sh
backup$ ./init.sh
```

Note that for the most common use case, that is pushing something and immediately pulling it again, you can omit the
generated ID entirely.  `scratch pull` defaults to the latest pushed file (as in the last example).


## Installation

You can download a compiled binary [here](https://github.com/kilobytetools/scratch-cli/releases/tag/stable).  Each
stable release is built for three target platforms:

* `x86_64-unknown-linux-gnu` (linux 64bit)
* `x86_64-apple-darwin` (mac 64bit)
* `x86_64-pc-windows-msvc` (windows 64bit)

You can also [compile from source](#compiling-from-source) which is based on the steps in
[this github action](https://github.com/kilobytetools/scratch-cli/blob/main/.github/workflows/build.yml).


## Setup

The cli contains a `bootstrap` command which uses your handle and password to generate a configuration
file at `~/.kilobytetools/config.toml`

```sh
$ scratch bootstrap
Enter your handle: numberoverzero
Enter your password:

# test our credentials by checking our usage
$ scratch stats --out-format json
{"max_bytes": 94371840, "max_files": 1024, "used_bytes": 0, "used_files": 0}
```


## Usage

The cli comes with help text for every command.  Here is the output of `scratch help`:

```
$ scratch help

USAGE: scratch [OPTIONS] [COMMAND]

Easily transmit small bits of short-lived data.

OPTIONS:
    --api-key API_KEY       API Key found in your account settings page.
    --endpoint ENDPOINT     Endpoint for dataplane operations, found in
                            your account settings page.
    --out-format FORMAT     Control how responses are rendered.  Allowed
                            values [text/plain, text/javascript, txt, js]

COMMAND:
    push        Upload the contents of a file
    pull        Get the contents of a file
    ls          List all file metadata
    rm          Remove a file by id
    stats       Get usage stats for your account
    bootstrap   Create a valid config file
```


## Flags and Defaults

Options are set according to the following precedence:

1. A flag passed to the cli directly (last value wins)
2. A value in the config file at `~/.kilobytetools/config`
3. The dataplane uses its default value

For example, the `prefix` option for `scratch push` would first look for `--prefix my-prefix`, then fall back to
the `prefix` setting in the config file under the `[scratch-push]` section (below), and finally default to the
dataplane's choice (no prefix).

```toml
[scratch-push]
lifetime = "25s"
prefix = "my-other-prefix"
```

Because last value wins, the following uses the prefix `"bar"`:

```sh
$ scratch push --prefix "foo" --prefix "" --prefix "bar" <<< "https://twitter.com/gitlost/status/1566348350550597633"
```

### Negating default values

You can use `--no-[FLAG]` to negate any boolean flag.  For example, if your config file defaults pushes to burn:

```toml
[scratch-push]
burn = true
```

You could use the following to push a file which can be read multiple times:

```sh
$ scratch push --no-burn < Dockerfile
```

Because last flag wins, the following will set `--burn`:

```sh
$ scratch push --burn --public --no-burn --lifetime 5m --burn < Dockerfile
```


## Compiling from source

Downloading scratch and self-bootstrapping (`scratch bootstrap`) is a common pattern, so we try to
keep the release binaries small.  Who wants to wait for a 40MiB binary just to move a 2KiB init script?

If you build on rust stable with `cargo build` or `cargo build --release` your resulting binaries will probably
be larger than the stable releases here.  To get similar results, you need to install rust nightly and use the
following steps.  These are taken from the
[github action](https://github.com/kilobytetools/scratch-cli/blob/main/.github/workflows/build.yml) in this repository.

Replace `[YOUR TARGET TRIPLE]` below with your
[target triple](https://doc.rust-lang.org/nightly/rustc/platform-support.html).  Note that cross compilation may
require additional steps (not provided).

```
rustup update nightly
rustup target add --toolchain nightly [YOUR TARGET TRIPLE]
rustup component add --toolchain nightly rust-src
cargo +nightly build -Z build-std=std,panic_abort -Z build-std-features=panic_immediate_abort --target [YOUR TARGET TRIPLE] --release
```
