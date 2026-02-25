# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

This is a PC Audio Mixer project using a Raspberry Pi Pico (RP2040) microcontroller. The firmware reads analog values from 3 slide potentiometers and sends the values to a PC via USB CDC (serial communication) in JSON format.

## Build and Development Commands

### Rust/Embedded Development

```bash
# Build the embedded firmware
cargo build --release

# Flash and run on the Pico with debugging output
cargo run

# Build without flashing
cargo build

# Alternative flashing with cargo-embed (if probe-rs is not available)
cargo embed
```

### Python Test Script

```bash
# Run the test script to monitor potentiometer values (using uv)
uv run test_pico_connection.py

# Or with pip
pip install pyserial
python test_pico_connection.py
```

## Architecture

### Hardware Configuration
- **Microcontroller**: Raspberry Pi Pico (RP2040)
- **ADC Inputs**:
  - Pot 1: GPIO26 (ADC0)
  - Pot 2: GPIO27 (ADC1)
  - Pot 3: GPIO28 (ADC2)
- **Communication**: USB CDC Serial at 115200 baud
- **Update Rate**: 20Hz (50ms delay between readings)

### Code Structure

**Main Firmware** (`src/main.rs`):
- Uses `rp-pico` BSP for hardware abstraction
- Implements USB CDC serial communication via `usbd-serial`
- Reads ADC values using embedded-hal traits
- Serializes data to JSON using `serde-json-core` (no_std compatible)
- Uses defmt for debug logging via probe-rs

**Alternative Implementation** (`src/main_mcp3008.rs`):
- Provides support for external MCP3008 ADC chip for additional channels

**Test Script** (`test_pico_connection.py`):
- Auto-detects Pico USB serial port
- Parses JSON data stream
- Displays real-time potentiometer values

### Key Dependencies
- `rp-pico`: Board support package
- `cortex-m` & `cortex-m-rt`: ARM Cortex-M runtime
- `embedded-hal`: Hardware abstraction traits
- `usb-device` & `usbd-serial`: USB CDC implementation
- `serde` & `serde-json-core`: JSON serialization (no_std)
- `defmt`: Efficient logging for embedded systems

## Development Setup

The project uses:
- **Target**: `thumbv6m-none-eabi` (ARM Cortex-M0+)
- **Runner**: `probe-rs run --chip RP2040 --protocol swd`
- **Linker**: `flip-link` for stack overflow protection
- **Debug**: defmt logging at debug level

## Data Format

The Pico sends JSON messages over USB serial:
```json
{"pot1":1234,"pot2":2345,"pot3":3456}
```
Each message is terminated with a newline character for easy parsing.