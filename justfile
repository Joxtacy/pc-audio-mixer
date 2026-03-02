build-win:
    cargo xwin build --target x86_64-pc-windows-msvc --release -p gui

build-firmware:
    cd firmware
    cargo build --release --bin firmware
    cd ..
