# srvr

Simple HTTP file server

> So simple, even the vowels are not needed

## Features

- Supports gzipped/brotlied files next to regular file
- All files are kept in memory to reduce disk access

## Usage

### Local install

Via Cargo
```sh
cargo install srvr
```

Or use the git repo for the latest version
```sh
git clone git@github.com:justim/srvr.git
cd srvr
cargo install --path .
```

When the binary is available, using it is simple; everything is optional:

```text
Serve files in a directory on a HTTP endpoint

Usage: srvr [OPTIONS] [BASE_DIR]

Arguments:
  [BASE_DIR]  The directory to serve to the world [default: .]

Options:
  -a, --address <ADDRESS>  The address to run srvr on, defaults to 127.0.0.1:12234
  -p, --port <PORT>        The port to run srvr on, defaults to 12234 (overrides `address`)
  -h, --help
```

### Docker

Runnig with Docker is also possible; make sure to expose the port and inject a volume.

```sh
docker build --tag srvr .
docker run --rm --interactive --tty --publish 12234:80 --volume ./:/var/srvr srvr
```

#### License

<sup>
Licensed under either of <a href="LICENSE-APACHE">Apache License, Version
2.0</a> or <a href="LICENSE-MIT">MIT license</a> at your option.
</sup>

<br>

<sub>
Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in this crate by you, as defined in the Apache-2.0 license, shall
be dual licensed as above, without any additional terms or conditions.
</sub>
