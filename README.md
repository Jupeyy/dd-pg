wasm:
```
rustup target add wasm32-unknown-unknown
cargo build --target wasm32-unknown-unknown -p <proj>
```

bundle:
```
cargo install cargo-bundle
cargo install cargo-outdated
```


asan:
```
RUSTFLAGS="-Z sanitizer=address" cargo run --target x86_64-unknown-linux-gnu
```
