wasm:
rustup target add wasm32-unknown-unknown
cargo build --target wasm32-unknown-unknown -p <proj>

symbolic link the wasm build dir to the config dir mod dir:
cargo watch -x "build --target wasm32-unknown-unknown -p <proj> --release && mv target/wasm32-unknown-unknown/release/<name>.wasm /home/jupeyy/.config/ddnet/mods/ui/wasm/wasm.wasm"

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
