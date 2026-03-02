#!/usr/bin/env python3
# /// script
# dependencies = ["pyserial>=3.5"]
# ///
"""
PC Audio Mixer - Pico Connection Test Script

This script automatically detects and connects to the Raspberry Pi Pico
running the audio mixer firmware, then displays real-time potentiometer values.

Usage with uv:
    uv run test_pico_connection.py

Usage with pip:
    pip install pyserial
    python test_pico_connection.py
"""

import serial
import serial.tools.list_ports
import json
import time
import sys

def find_pico_port():
    """Automatically find the Pico device port"""
    print("Searching for Pico device...")
    ports = serial.tools.list_ports.comports()

    # Look for common Pico identifiers
    for port in ports:
        # Check for known Pico descriptions/manufacturers
        if any(keyword in str(port.description).lower() for keyword in
               ['pico', 'rp2040', 'usb serial']):
            print(f"Found Pico: {port.device} - {port.description}")
            return port.device

        # Check for USB modem pattern (common on macOS)
        if 'usbmodem' in port.device.lower():
            print(f"Found USB modem device: {port.device} - {port.description}")
            return port.device

    # Fallback: let user choose
    if ports:
        print("\nAvailable ports:")
        for i, port in enumerate(ports):
            print(f"  {i}: {port.device} - {port.description}")

        try:
            choice = input("\nEnter port number (or 'q' to quit): ")
            if choice.lower() == 'q':
                return None
            return ports[int(choice)].device
        except (ValueError, IndexError):
            print("Invalid selection")
            return None

    print("No serial ports found!")
    return None

def connect_to_pico():
    """Connect to Pico with auto-discovery"""
    port = find_pico_port()
    if not port:
        print("No Pico found!")
        return None

    try:
        ser = serial.Serial(port, 115200, timeout=1)
        print(f"✅ Connected to Pico on {port}")
        print("Waiting for data...\n")
        return ser
    except serial.SerialException as e:
        print(f"❌ Failed to connect to {port}: {e}")
        return None

def main():
    print("PC Audio Mixer - Pico Connection Test")
    print("=" * 40)

    # Connect to Pico
    ser = connect_to_pico()
    if not ser:
        print("Exiting...")
        return

    print("📊 Real-time potentiometer values:")
    print("   (Move your potentiometers to see values change)")
    print("   (Press Ctrl+C to exit)")
    print()

    try:
        line_count = 0
        while True:
            if ser.in_waiting > 0:
                line = ser.readline().decode('utf-8').strip()
                try:
                    data = json.loads(line)
                    # Use default value 0 for any missing potentiometer
                    pot_values = {
                        'pot1': data.get('pot1', -1),
                        'pot2': data.get('pot2', -1),
                        'pot3': data.get('pot3', -1),
                        'pot4': data.get('pot4', -1),
                        'pot5': data.get('pot5', -1),
                        'pot6': data.get('pot6', -1),
                        'pot7': data.get('pot7', -1),
                        'pot8': data.get('pot8', -1)
                    }
                    # Clear previous line and print new values
                    print(f"\r🎛️  Pot1: {pot_values['pot1']:4d}  |  Pot2: {pot_values['pot2']:4d}  |  Pot3: {pot_values['pot3']:4d}  |  Pot4: {pot_values['pot4']:4d} |  Pot5: {pot_values['pot5']:4d} |  Pot6: {pot_values['pot6']:4d} |  Pot7: {pot_values['pot7']:4d} |  Pot8: {pot_values['pot8']:4d}      ", end="", flush=True)
                    line_count += 1

                    # Print a newline occasionally for readability
                    if line_count % 20 == 0:
                        print()  # New line every 20 updates

                except json.JSONDecodeError:
                    print(f"Raw data received: {line}")
                except KeyError as e:
                    print(f"Missing key in data: {e} - Raw: {line}")

            time.sleep(0.01)  # Small delay to prevent excessive CPU usage

    except KeyboardInterrupt:
        print("\n\n👋 Disconnecting from Pico...")
        ser.close()
        print("Connection closed. Goodbye!")

    except serial.SerialException as e:
        print(f"\n❌ Connection error: {e}")
        print("Check if the Pico is still connected and try again.")

if __name__ == "__main__":
    main()
