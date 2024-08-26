wasm:
```
rustup target add wasm32-unknown-unknown
cargo build --target wasm32-unknown-unknown -p <proj>
```
wasm release:
```
cargo +nightly build --target wasm32-unknown-unknown -p <proj> -Z build-std=panic_abort,std
```
(this allows full simd instructions)

symbolic link the wasm build dir to the config dir mod dir:
```
cargo watch -x "build --target wasm32-unknown-unknown -p <proj> --release" -s "cp target/wasm32-unknown-unknown/release/<name>.wasm ~/.config/ddnet/mods/ui/wasm/wasm.wasm"
```

Windows:

```
cargo watch -x "build --target wasm32-unknown-unknown -p <proj> --release" -s "xcopy target\wasm32-unknown-unknown\release\<name>.wasm $env:AppData\DDNet\config\mods\ui\wasm\wasm.wasm /Y"
```

if cargo watch is slow (some versions are broken):
https://github.com/watchexec/cargo-watch/issues/276
`cargo install cargo-watch --locked --version 8.1.2`

Android:
```
# tree
ANDROID_NDK_HOME=/home/jupeyy/Android/Sdk/ndk/26.3.11579264/ cargo ndk -t arm64-v8a tree --no-default-features

# build, (currently needs hack, remove line `default = ["legacy"]` in Cargo.toml)
x build --arch arm64 --platform android --format apk -p ddnet-playground --features bundled_data_dir
```

network jitter:
sudo tc qdisc add dev lo root netem delay 100ms 10ms 
sudo tc qdisc del dev lo root

bundle:
cargo install cargo-bundle
cargo install cargo-outdated
asan:
RUSTFLAGS="-Z sanitizer=address" cargo run --target x86_64-unknown-linux-gnu
TSAN_OPTIONS="ignore_noninstrumented_modules=1" RUSTFLAGS="-Z sanitizer=thread" cargo run --target x86_64-unknown-linux-gnu

cargo stuff (for CI maybe):
cargo-udeps
cargo-edit
cargo-outdated
cargo-geiger
cargo-license
cargo-crev
cargo-bloat
cargo-machete
cargo-upgrade

x11 mouse cursor while debugging:
install xdotool package
if you use the vscode workspace in other/vscode it will do the following steps automatically

lldb has to execute this add start of debugging:

```
command source ${env:HOME}/.lldbinit
```

in ~/.lldbinit:
```
target stop-hook add --one-liner "command script import  ~/lldbinit.py"
``

in ~/lldbinit.py (no dot!):
```
#!/usr/bin/python
import os

print("Breakpoint hit!")
os.system("setxkbmap -option grab:break_actions")
os.system("xdotool key XF86Ungrab")
```
