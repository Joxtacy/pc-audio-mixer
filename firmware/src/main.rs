//! PC Audio Mixer - Reads 3 slide potentiometers and sends values over USB
//!
//! This application reads analog values from 3 potentiometers connected to ADC pins
//! and transmits their values to a PC over USB CDC (serial communication).
//!
//! Potentiometer connections:
//! - Pot 1: GPIO26 (ADC0)
//! - Pot 2: GPIO27 (ADC1)
//! - Pot 3: GPIO28 (ADC2)
//!
//! Note: GPIO29 (ADC3) is not available on this board.
//! For additional channels, consider using an external ADC like MCP3008.

#![no_std]
#![no_main]

use bsp::entry;
use defmt::*;
use defmt_rtt as _;
use panic_probe as _;

// Provide an alias for our BSP so we can switch targets quickly.
use rp_pico as bsp;

use bsp::hal::{
    adc::{Adc, AdcPin},
    clocks::{init_clocks_and_plls, Clock},
    pac,
    sio::Sio,
    usb::UsbBus,
    watchdog::Watchdog,
};

// Import nb trait for non-blocking operations
use nb::block;
// Import embedded-hal v0.2 traits
use embedded_hal::adc::OneShot;

use usb_device::{class_prelude::*, prelude::*};
use usbd_serial::{SerialPort, USB_CLASS_CDC};

use serde::Serialize;

// Structure to hold potentiometer readings
#[derive(Serialize)]
struct PotentiometerData {
    pot1: u16,
    pot2: u16,
    pot3: u16,
}

#[entry]
fn main() -> ! {
    info!("PC Audio Mixer starting...");

    // Take ownership of the device peripherals
    let mut pac = pac::Peripherals::take().unwrap();
    let core = pac::CorePeripherals::take().unwrap();

    // Set up the watchdog driver
    let mut watchdog = Watchdog::new(pac.WATCHDOG);

    // Configure the clocks
    let external_xtal_freq_hz = 12_000_000u32;
    let clocks = init_clocks_and_plls(
        external_xtal_freq_hz,
        pac.XOSC,
        pac.CLOCKS,
        pac.PLL_SYS,
        pac.PLL_USB,
        &mut pac.RESETS,
        &mut watchdog,
    )
    .ok()
    .unwrap();

    // Set up the USB driver
    let usb_bus = UsbBusAllocator::new(UsbBus::new(
        pac.USBCTRL_REGS,
        pac.USBCTRL_DPRAM,
        clocks.usb_clock,
        true,
        &mut pac.RESETS,
    ));

    // Set up the USB Communications Class Device driver (CDC/Serial)
    let mut serial = SerialPort::new(&usb_bus);

    // Create a USB device with a fake VID and PID
    let mut usb_dev = UsbDeviceBuilder::new(&usb_bus, UsbVidPid(0x16c0, 0x27dd))
        .device_class(USB_CLASS_CDC)
        .build();

    // Set up the GPIO pins
    let sio = Sio::new(pac.SIO);
    let pins = bsp::Pins::new(
        pac.IO_BANK0,
        pac.PADS_BANK0,
        sio.gpio_bank0,
        &mut pac.RESETS,
    );

    // Initialize the ADC
    let mut adc = Adc::new(pac.ADC, &mut pac.RESETS);

    // Configure ADC pins for potentiometers
    // Using 3 available ADC inputs: ADC0 (GPIO26), ADC1 (GPIO27), ADC2 (GPIO28)
    let mut adc_pin_0 = AdcPin::new(pins.gpio26.into_floating_input()).unwrap();
    let mut adc_pin_1 = AdcPin::new(pins.gpio27.into_floating_input()).unwrap();
    let mut adc_pin_2 = AdcPin::new(pins.gpio28.into_floating_input()).unwrap();

    // Set up timing for regular readings
    let mut delay = cortex_m::delay::Delay::new(core.SYST, clocks.system_clock.freq().to_Hz());

    info!("Setup complete, starting main loop...");

    loop {
        // Poll the USB device
        if usb_dev.poll(&mut [&mut serial]) {
            // Handle any USB events
        }

        // Read all potentiometer values
        let pot1_raw: u16 = block!(adc.read(&mut adc_pin_0)).unwrap();
        let pot2_raw: u16 = block!(adc.read(&mut adc_pin_1)).unwrap();
        let pot3_raw: u16 = block!(adc.read(&mut adc_pin_2)).unwrap();

        // Create the data structure
        let pot_data = PotentiometerData {
            pot1: pot1_raw,
            pot2: pot2_raw,
            pot3: pot3_raw,
        };

        // Serialize to JSON string
        if let Ok(json_string) = serde_json_core::to_string::<_, 256>(&pot_data) {
            let mut full_message = json_string;
            // Add newline for easier parsing on PC side
            full_message.push('\n').ok();

            // Send over USB serial
            let _ = serial.write(full_message.as_bytes());

            info!("Sent: {}", full_message.as_str());
        }

        // Wait 50ms between readings (20Hz update rate)
        // This provides smooth control without overwhelming the USB connection
        delay.delay_ms(50);
    }
}
