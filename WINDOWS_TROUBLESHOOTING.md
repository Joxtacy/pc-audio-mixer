# Windows Troubleshooting Guide

## If the app is stuck "Initializing"

### 1. Check Console Output
- The app will now automatically open DevTools in debug builds
- If DevTools don't open automatically, try:
  - Press `F12` or `Ctrl+Shift+I`
  - Right-click and select "Inspect"

### 2. Common Issues and Solutions

#### Issue: "Tauri context not available"
**Cause**: The Tauri runtime isn't injecting properly into the webview
**Solutions**:
- Ensure you have WebView2 Runtime installed (usually comes with Windows 10/11)
- Try reinstalling the app
- Check Windows Event Viewer for errors

#### Issue: "localhost refused to connect"
**Cause**: The app is trying to load from a dev server instead of bundled files
**Solutions**:
- This is a build configuration issue - the app was built incorrectly
- Use the official release from GitHub Actions
- Build locally on Windows instead of cross-compiling

#### Issue: No audio sessions detected
**Cause**: COM initialization failed or permissions issue
**Solutions**:
- Run the app as Administrator (right-click → Run as Administrator)
- Check that Windows Audio Service is running
- Restart Windows Audio Service:
  ```cmd
  net stop audiosrv
  net start audiosrv
  ```

### 3. Manual Debugging Steps

1. **Check if Tauri is loading**:
   - Open the app
   - Within 10 seconds, you should see either:
     - The main UI
     - An error message
   - If stuck on "Initializing" for more than 10 seconds, check console

2. **Enable Console Output**:
   Set environment variable before running:
   ```cmd
   set RUST_LOG=info
   gui.exe
   ```

3. **Check WebView2**:
   - Open Edge browser and go to: `edge://version`
   - Look for "WebView2" version
   - If missing, download from: https://developer.microsoft.com/microsoft-edge/webview2/

### 4. What's Changed

Recent fixes include:
- Added 10-second timeout to prevent infinite initialization
- DevTools automatically open in debug builds
- Better error messages and logging
- Fallback mode if audio sessions can't load
- More detailed console output during initialization

### 5. Build from Source on Windows

If the pre-built version doesn't work:

```bash
# Install prerequisites
# - Node.js: https://nodejs.org/
# - Rust: https://rustup.rs/
# - pnpm: npm install -g pnpm

# Clone and build
git clone https://github.com/yourusername/pc-audio-mixer.git
cd pc-audio-mixer/gui
pnpm install
pnpm tauri build

# For debug build with console:
set RUST_LOG=info
pnpm tauri dev
```

### 6. Report Issues

If still having problems, please report with:
1. Screenshot of the stuck screen
2. Console output (F12 → Console tab)
3. Windows version (Win+R → winver)
4. Whether you built from source or used pre-built
5. Any error messages from Windows Event Viewer