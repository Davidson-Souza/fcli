# fcli - A drop-in replacement for CLN's bcli plugin using florestad

> This project is under development, expect bugs

Core Lightning (a.k.a. CLN) is an implementation of the [Lightning Network](https://lightning.network/) protocol. Lightning is meant to be anchored in some blockchain, like Bitcoin. Therefore, any implementation needs to fetch data related to this blockchain.

CLN uses a plugin to do so. This plugin have to implement some basic interface defined by `lightningd`. Any plugin that conforms with that interface is fine. This plugin is an implementation of that, but using [Floresta](https://github.com/Davidson-Souza/floresta): a lightweight full node implementation, that is very resource efficient.

## Building

You need rust to build this project. Assuming you have `cargo` installed, just run

```bash
$ cargo build --release
```

## Running

Before running this, you need to start `florestad`, see [floresta's](https://github.com/Davidson-Souza/floresta) build system. After you have that running, just start `lightningd` with those options:

```bash
$ lightningd --disable-plugin bcli --plugin <path-to-fcli>
```

the final binary will be inside `target/release`. So, if you're working on `$HOME/fcli`, you'll use

```bash
$ lightningd --disable-plugin bcli --plugin $HOME/fcli/target/release/fcli
```

