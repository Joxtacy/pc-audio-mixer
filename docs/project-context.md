---
project_name: 'pc-audio-mixer'
user_name: 'Joxtacy'
date: '2026-02-25'
sections_completed:
  ['technology_stack', 'language_rules', 'framework_rules', 'testing_rules', 'quality_rules', 'workflow_rules', 'anti_patterns']
status: 'complete'
rule_count: 85
optimized_for_llm: true
---

# Project Context for AI Agents

_This file contains critical rules and patterns that AI agents must follow when implementing code in this project. Focus on unobvious details that agents might otherwise miss._

---

## Technology Stack & Versions

### Core Platform
- Rust Edition 2021 (no_std embedded environment)
- Target: thumbv6m-none-eabi (ARM Cortex-M0+)
- Raspberry Pi Pico with RP2040 microcontroller
- Clock: 12MHz external crystal

### Required Dependencies (exact versions critical for embedded)
- rp-pico = "0.9" (Board Support Package)
- cortex-m = "0.7" & cortex-m-rt = "0.7"
- embedded-hal = { version = "0.2.7", features = ["unproven"] }
- usb-device = "0.3.2" & usbd-serial = "0.2.2"
- serde = { version = "1.0", default-features = false, features = ["derive"] }
- serde-json-core = "0.5.1" (no_std JSON serialization)
- defmt = "1" & defmt-rtt = "1" & panic-probe = { version = "1", features = ["print-defmt"] }
- nb = "1.1" (non-blocking traits)
- heapless = { version = "0.8", features = ["serde"] }

### Build Configuration
- Runner: probe-rs run --chip RP2040 --protocol swd
- Linker: flip-link
- DEFMT_LOG: debug level

## Critical Implementation Rules

### Language-Specific Rules (Embedded Rust)

- MUST use `#![no_std]` and `#![no_main]` attributes in main.rs
- Entry point MUST use `#[entry]` attribute from cortex-m-rt
- NO heap allocation - use heapless collections or static buffers
- Buffer sizes MUST be const generics (e.g., `heapless::String<256>`)
- Use `defmt::info!`, `defmt::error!` etc. for logging (NOT println!)
- BSP alias pattern required: `use rp_pico as bsp;`
- Access HAL through BSP: `use bsp::hal::{module}` not direct rp2040-hal
- Use `nb::block!` to convert nb::Result to blocking
- Peripheral init errors can `.unwrap()` - panic-probe will handle
- NEVER use std library features or allocator-dependent code
- Prefer embedded-hal traits over direct register manipulation
- Use `cortex_m::delay::Delay` for timing, not busy loops

### Framework-Specific Rules (RP2040/rp-pico BSP)

- Peripherals obtained via `pac::Peripherals::take().unwrap()` ONCE only
- Clock init sequence: external_xtal → init_clocks_and_plls → system clocks
- External crystal frequency: 12_000_000 Hz (12MHz)
- USB initialization requires: USBCTRL_REGS, USBCTRL_DPRAM, usb_clock
- USB VID/PID: 0x16c0/0x27dd for CDC serial device
- Serial communication: 115200 baud, USB CDC class
- GPIO pins accessed through BSP Pins structure, not raw registers
- ADC: Use external MCP3008 via SPI (8 channels, 10-bit resolution)
- MCP3008 SPI: Mode 0, max 3.6MHz at 5V, 2.0MHz at 3.3V
- MCP3008 requires: SPI MOSI, MISO, SCK, and CS pin
- Do NOT use internal ADC pins (GPIO26/27/28) - reserved for MCP3008 SPI
- JSON messages MUST end with '\n' for PC-side parsing
- Update rate: 50ms delay between readings (20Hz)
- Always initialize Watchdog even if not used (required for clocks)
- Reference src/main_mcp3008.rs for MCP3008 implementation patterns

### Testing Rules

- Embedded code tested via hardware-in-the-loop with Python scripts
- Python test script MUST auto-detect Pico USB port
- Test scripts use pyserial for USB CDC communication
- Validate JSON format: {"pot1":value,"pot2":value,...} with newline
- Test ADC value range: 0-1023 for MCP3008 (10-bit)
- Integration tests via test_pico_connection.py or similar
- Use defmt-test for on-target unit tests if needed
- Mock hardware using embedded-hal traits for off-target testing
- Test error conditions: USB disconnect, invalid ADC readings
- Verify 20Hz update rate (50ms ± 10ms between messages)
- Always test with actual hardware before considering complete

### Code Quality & Style Rules

- Run `cargo fmt` before committing - enforces Rust formatting standards
- Run `cargo clippy` for additional linting and best practices
- Module docs with //! explaining hardware setup and pin connections
- Use snake_case for variables/functions, PascalCase for types
- UPPER_CASE for const values (e.g., EXTERNAL_XTAL_FREQ)
- Single main.rs for primary implementation (no lib.rs for binaries)
- Alternative mains as separate files (e.g., main_mcp3008.rs)
- Group imports: extern crates → use statements → local modules
- Comment non-obvious embedded operations (e.g., why specific delays)
- Document ALL pin assignments with GPIO numbers and functions
- Keep Python test scripts at project root with descriptive names
- Use type aliases for clarity (e.g., `type AdcReading = u16;`)
- Explicit error handling in init, panic in main loop acceptable

### Development Workflow Rules

- Use Jujutsu (jj) as VCS - NOT git directly
- Use probe-rs for flashing: `cargo run` (configured in .cargo/config.toml)
- Alternative: cargo-embed if probe-rs unavailable
- Build release version: `cargo build --release` for production firmware
- Flash release: `cargo run --release` for optimized binary
- Test hardware connection with Python script before firmware changes
- Run `cargo fmt` and `cargo clippy` before commits
- Follow Conventional Commits format in jj describe: type(scope): description
  - Types: feat, fix, docs, style, refactor, test, chore
  - Example: `feat(adc): add MCP3008 SPI support for 8 channels`
  - Example: `fix(usb): resolve disconnect handling in main loop`
- Use `jj describe` to set commit messages, not git commit
- Document GPIO changes in commit body when pin mappings modified
- Update CLAUDE.md when hardware connections change
- Keep debug probe connected during development for defmt output
- Monitor defmt logs: critical for embedded debugging
- Test both debug and release builds - timing can differ

### Critical Don't-Miss Rules

- NEVER import std library - will fail to compile for no_std target
- NEVER use heap allocation (Box, Vec, String) - use heapless alternatives
- NEVER call peripheral take() more than once - causes panic
- NEVER block indefinitely - watchdog will reset the system
- NEVER use internal ADC pins if MCP3008 configured - they conflict
- NEVER exceed buffer sizes - heapless has fixed capacity (e.g., 256 bytes)
- SPI clock for MCP3008: max 2MHz at 3.3V (Pico voltage level)
- Handle USB disconnect gracefully - don't panic on write failures
- Validate ADC readings: 0-1023 range for MCP3008 10-bit values
- JSON serialization can fail with large messages - keep under 256 chars
- Don't put delays in interrupt handlers - breaks timing
- Always check Option<T> from take() before unwrap() in examples
- Memory.x defines RAM/FLASH layout - don't modify without understanding
- defmt messages visible to anyone with probe access - no secrets
- Test edge cases: all pots at min/max simultaneously

---

## Usage Guidelines

**For AI Agents:**
- Read this file before implementing any code
- Follow ALL rules exactly as documented
- When in doubt, prefer the more restrictive option
- Update this file if new patterns emerge

**For Humans:**
- Keep this file lean and focused on agent needs
- Update when technology stack changes
- Review quarterly for outdated rules
- Remove rules that become obvious over time

Last Updated: 2026-02-25