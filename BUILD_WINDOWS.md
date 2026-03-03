# Building on Windows

## Prerequisites

1. Install Node.js (https://nodejs.org/)
2. Install Rust (https://rustup.rs/)
3. Install pnpm: `npm install -g pnpm`
4. Install Microsoft C++ Build Tools (if not already installed)

## Build Steps

1. Clone the repository on Windows
2. Navigate to the gui directory: `cd gui`
3. Install dependencies: `pnpm install`
4. Build the application: `pnpm tauri build`

## Development Mode

To run in development mode with console output:
1. Set environment variable: `set RUST_LOG=info`
2. Run: `pnpm tauri dev`

## Troubleshooting

If the app shows "localhost refused to connect":
- Make sure the build completed successfully
- Check that the `build` directory exists with index.html
- Try running `pnpm build` separately first, then `pnpm tauri build`

If the app is stuck "initializing":
- Open Developer Tools (if available) and check the console
- Look for error messages about Tauri context or audio sessions
- On Windows, the app needs COM initialization to work with audio

## Testing the Changes

The recent changes added:
1. Better error handling during initialization
2. Fallback mode if audio sessions can't be loaded
3. Console logging for debugging

When running the app, check the console for messages like:
- "Tauri context ready"
- "COM initialized successfully" (Windows only)
- "Audio sessions loaded: X sessions found"