#!/usr/bin/env python3
# /// script
# dependencies = ["pyserial>=3.5"]
# ///
"""
Debug script to check USB devices and serial ports
"""

import serial.tools.list_ports
import subprocess
import sys

def check_usb_devices():
    """Check for USB devices using system_profiler"""
    print("🔍 Checking USB devices...")
    try:
        result = subprocess.run(['system_profiler', 'SPUSBDataType'],
                              capture_output=True, text=True)

        lines = result.stdout.split('\n')
        found_pico = False

        for i, line in enumerate(lines):
            if any(keyword in line.lower() for keyword in ['pico', 'rp2040', 'raspberry']):
                print("📱 Found Pico-related device:")
                # Print context around the match
                for j in range(max(0, i-3), min(len(lines), i+5)):
                    print(f"   {lines[j]}")
                print()
                found_pico = True

        if not found_pico:
            print("❌ No Pico-related USB devices found")

    except Exception as e:
        print(f"Error running system_profiler: {e}")

def check_serial_ports():
    """Check all serial ports with detailed info"""
    print("\n🔌 Available serial ports:")
    ports = serial.tools.list_ports.comports()

    if not ports:
        print("❌ No serial ports found")
        return

    for port in ports:
        print(f"📍 Port: {port.device}")
        print(f"   Description: {port.description}")
        print(f"   Manufacturer: {port.manufacturer}")
        print(f"   VID:PID: {port.vid}:{port.pid}")
        print(f"   Serial Number: {port.serial_number}")
        print()

def check_cu_devices():
    """Check /dev/cu.* devices"""
    print("🔍 Checking /dev/cu.* devices:")
    try:
        result = subprocess.run(['ls', '/dev/cu.*'],
                              capture_output=True, text=True, shell=True)
        if result.returncode == 0:
            devices = result.stdout.strip().split('\n')
            for device in devices:
                print(f"   {device}")
        else:
            print("   No /dev/cu.* devices found")
    except Exception as e:
        print(f"   Error: {e}")

def main():
    print("PC Audio Mixer - USB Debug Tool")
    print("=" * 40)

    check_usb_devices()
    check_serial_ports()
    check_cu_devices()

    print("\n💡 Troubleshooting tips:")
    print("1. If no Pico found: Check USB cable and try different port")
    print("2. If Pico found but no serial: Flash the firmware first")
    print("3. If multiple serial ports: Try each one with the test script")

if __name__ == "__main__":
    main()