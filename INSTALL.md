# INSTALL.md

## Requirements for building application

```
$ sudo apt update 

$ curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

$ sudo apt install build-essential pkg-config bpftool libssl-dev -y

$ source "$HOME/.cargo/env"

$ bash -c "$(wget -O - https://apt.llvm.org/llvm.sh)"

$ rustup install stable

$ rustup toolchain install nightly --component rust-src

$ cargo install bpf-linker

$ cargo install cargo-generate
```


## Disable checksums on the network interface

```
$ ethtool -K eth0 rx-checksumming off tx-checksumming off
```

