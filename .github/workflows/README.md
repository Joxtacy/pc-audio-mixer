# GitHub Actions Workflows

This directory contains automated workflows for building and releasing the PC Audio Mixer application.

## Workflows

### 1. Build and Release (`build-release.yml`)
**Purpose**: Builds the application for all platforms and creates releases

**Triggers**:
- Push to `main` or `master` branch
- Pull requests
- Git tags starting with `v` (e.g., `v1.0.0`)
- Manual trigger with optional release creation

**What it does**:
- Builds for Windows (x64), macOS (Intel & Apple Silicon), and Linux
- Creates installers (.exe, .msi, .dmg, .deb, .AppImage)
- Uploads artifacts for download
- Creates GitHub releases for version tags
- Also builds the Pico firmware

### 2. Continuous Integration (`ci.yml`)
**Purpose**: Quick checks on every commit

**Triggers**:
- Every push to main/master
- Every pull request

**What it does**:
- Runs Rust tests on all platforms
- Checks code formatting (rustfmt)
- Runs linter (clippy)
- Type-checks the frontend

## How to Use

### Getting Build Artifacts

1. **From Any Commit**:
   - Go to the [Actions tab](../../actions)
   - Click on a workflow run
   - Scroll to "Artifacts" section
   - Download the installer for your platform

2. **From Releases**:
   - Go to [Releases](../../releases)
   - Download the appropriate installer

### Creating a New Release

**Option 1: Using Git Tags**
```bash
# Create and push a version tag
git tag v1.0.0
git push origin v1.0.0
```
This automatically triggers a build and creates a draft release.

**Option 2: Manual Trigger**
1. Go to [Actions tab](../../actions)
2. Select "Build and Release PC Audio Mixer"
3. Click "Run workflow"
4. Check "Create a release"
5. Click "Run workflow"

### Platform-Specific Installers

| Platform | File Types | Description |
|----------|------------|-------------|
| Windows | `.exe`, `.msi` | NSIS installer (exe) or MSI package |
| macOS Intel | `.dmg` | Disk image for Intel Macs |
| macOS Apple Silicon | `.dmg` | Disk image for M1/M2/M3 Macs |
| Linux | `.deb`, `.AppImage` | Debian package or universal AppImage |

## Secrets Required (Optional)

For code signing (optional but recommended for distribution):

- `TAURI_SIGNING_PRIVATE_KEY`: Tauri update signing key
- `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`: Password for the signing key

To generate these:
```bash
pnpm tauri signer generate -w ~/.tauri/pc-audio-mixer.key
# Add the private key and password to GitHub Secrets
```

## Build Status Badges

Add these to your main README:

```markdown
![Build Status](https://github.com/joxtacy/pc-audio-mixer/workflows/Build%20and%20Release%20PC%20Audio%20Mixer/badge.svg)
![CI](https://github.com/joxtacy/pc-audio-mixer/workflows/CI/badge.svg)
```

## Troubleshooting

### Build Fails on Windows
- Usually due to Windows-specific dependencies
- Check that all Windows audio APIs are properly conditionally compiled

### Build Fails on macOS
- May need to accept Xcode license: `sudo xcodebuild -license accept`
- Ensure macOS deployment target is set correctly

### Build Fails on Linux
- Missing system dependencies
- The workflow installs common ones, but some distributions may need more

### Artifacts Not Uploading
- Check the artifact paths match your actual build output
- Verify the Tauri config generates the expected bundle types

## Local Testing

To test the workflows locally, you can use [act](https://github.com/nektos/act):

```bash
# Install act
brew install act  # macOS
# or download from https://github.com/nektos/act

# Run the CI workflow locally
act -j test

# Run the build workflow
act -j build --platform ubuntu-latest
```

## Customization

Feel free to modify these workflows for your needs:
- Change Node/Rust versions
- Add more platforms or architectures
- Include additional build steps
- Add deployment to app stores