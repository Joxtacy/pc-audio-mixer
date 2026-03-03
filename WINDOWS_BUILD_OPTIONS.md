# Windows Build Options from macOS

## Option 1: Build Executable Only (Easiest)
You can build the Windows .exe from macOS without installers:

```bash
cd gui
pnpm tauri build --target x86_64-pc-windows-msvc --bundles none
```

This creates a standalone .exe file at:
`target/x86_64-pc-windows-msvc/release/gui.exe`

You can distribute this .exe directly, but users will need to:
- Allow it through Windows Defender
- Manually create shortcuts
- No automatic updates

## Option 2: Use GitHub Actions (Recommended)
The repository already has GitHub Actions configured to build installers on Windows runners:

1. Push your changes to GitHub
2. Either:
   - Create a tag starting with `v` (e.g., `v0.1.0`) to trigger a release
   - Or manually trigger the workflow from GitHub Actions tab
3. GitHub Actions will build on actual Windows machines and create:
   - NSIS installer (.exe)
   - MSI installer (.msi)

## Option 3: Use Wine on macOS (Complex)
You can install Wine and NSIS on macOS to create Windows installers:

```bash
# Install Wine and NSIS using Homebrew
brew install --cask wine-stable
brew install makensis

# Then build with:
pnpm tauri build --target x86_64-pc-windows-msvc
```

Note: This is experimental and may not work reliably.

## Option 4: Use a Windows VM
Install Windows in a VM (Parallels, VMware, VirtualBox) and build there:
1. Install Windows 11 in VM
2. Install development tools (Node.js, Rust, pnpm)
3. Clone repository and build normally

## Option 5: Remote Windows Machine
Use a Windows cloud instance or remote machine:
- Azure Windows VM
- AWS EC2 Windows instance
- GitHub Codespaces with Windows runner

## Current Build Issues

The build might fail on macOS because:
1. `llvm-rc` is not available (needed for Windows resources)
2. Cross-compilation toolchain issues

To fix the llvm-rc issue, you can try:
```bash
# Install LLVM
brew install llvm

# Add to PATH
export PATH="/opt/homebrew/opt/llvm/bin:$PATH"
```

## Simplest Solution for Testing

For now, the simplest approach is:
1. Build the .exe without bundlers: `pnpm tauri build --target x86_64-pc-windows-msvc --bundles none`
2. Copy the .exe to your Windows machine
3. Test it there

For production releases, use GitHub Actions which handles all the complexity automatically.