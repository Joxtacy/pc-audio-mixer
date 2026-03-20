build-win:
    cargo xwin build --target x86_64-pc-windows-msvc --release -p gui

build-firmware:
    cd firmware && cargo build --release --bin firmware

build-uf2: build-firmware
    elf2uf2-rs target/thumbv6m-none-eabi/release/firmware firmware.uf2
