# 0x-mesh implemented in Rust

![lines of code](https://img.shields.io/tokei/lines/github/0xProject/mesh-rs)
[![dependency status](https://deps.rs/repo/github/0xProject/mesh-rs/status.svg)](https://deps.rs/repo/github/0xProject/mesh-rs)
[![codecov](https://img.shields.io/codecov/c/github/0xProject/mesh-rs)](https://codecov.io/gh/0xProject/mesh-rs)
[![build](https://img.shields.io/github/workflow/status/0xProject/mesh-rs/build)](https://github.com/0xProject/mesh-rs/actions?query=workflow%3Abuild)
[![deploy-gke](https://img.shields.io/github/workflow/status/0xProject/mesh-rs/deploy-gke)](https://github.com/0xProject/mesh-rs/actions?query=workflow%3Adeploy-gke)

## Quickstart

```
cargo run -- -vvv
```

Should connect to the 0xMesh main network and start logging order (among many other things).

## Blocking issues

* `/libp2p/circuit/relay/0.1.0` protocol support is currently unavailable in Rust libp2p.
  https://github.com/libp2p/rust-libp2p/issues/725
  https://github.com/libp2p/rust-libp2p/pull/1838
* NAT traversal is unavailable in Rust libp2p.
  https://github.com/libp2p/rust-libp2p/issues/1722
* 


## References

https://github.com/libp2p/rust-libp2p

https://docs.rs/libp2p/0.32.2/libp2p/
