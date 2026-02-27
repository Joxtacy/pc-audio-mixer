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

// The macro for our start-up function
use rp_pico::entry;

// Use panic_probe when debugging with probe, panic_halt otherwise
#[cfg(feature = "probe")]
use defmt::*;
#[cfg(feature = "probe")]
use defmt_rtt as _;
#[cfg(feature = "probe")]
use panic_probe as _;

#[cfg(not(feature = "probe"))]
use panic_halt as _;

// Peripheral Access Crate
use rp_pico::hal::pac;

// Hardware Abstraction Layer
use rp_pico::hal;

use hal::{
    adc::{Adc, AdcPin},
    clocks::init_clocks_and_plls,
};

// Import nb trait for non-blocking operations
use nb::block;
// Import embedded-hal v0.2 traits
use embedded_hal::adc::OneShot;
use embedded_hal::digital::v2::{OutputPin, StatefulOutputPin};

use usb_device::device::StringDescriptors;
use usb_device::{class_prelude::*, prelude::*};
use usbd_serial::SerialPort;

use core::fmt::Write;
use heapless::String;

use serde::Serialize;

// Structure to hold potentiometer readings
#[derive(Serialize)]
struct PotentiometerData {
    pot1: u16,
    pot2: u16,
    pot3: u16,
}

/// Drives the pin high
fn pin_on<P: OutputPin>(led: &mut P) -> Result<(), P::Error> {
    led.set_high()
}

/// Drives the pin low
fn pin_off<P: OutputPin>(led: &mut P) -> Result<(), P::Error> {
    led.set_low()
}

/// Toggles the state of the pin
fn pin_toggle<P: StatefulOutputPin>(led: &mut P) -> Result<(), P::Error>
where
    P::Error: core::fmt::Debug,
{
    if led.is_set_high().unwrap_or(false) {
        led.set_low()
    } else {
        led.set_high()
    }
}

#[entry]
fn main() -> ! {
    #[cfg(feature = "probe")]
    info!("PC Audio Mixer starting...");

    // Take ownership of the device peripherals
    let mut pac = pac::Peripherals::take().unwrap();
    let core = pac::CorePeripherals::take().unwrap();

    // Set up the watchdog driver - needed by the clock setup code
    let mut watchdog = hal::Watchdog::new(pac.WATCHDOG);

    let clock_speed = rp_pico::XOSC_CRYSTAL_FREQ;
    let clock_speed = 12_000_000u32;
    // Configure the clocks
    let clocks = init_clocks_and_plls(
        clock_speed,
        pac.XOSC,
        pac.CLOCKS,
        pac.PLL_SYS,
        pac.PLL_USB,
        &mut pac.RESETS,
        &mut watchdog,
    )
    .ok()
    .unwrap();

    // let _timer = hal::Timer::new(pac.TIMER, &mut pac.RESETS, &clocks);

    // Set up the USB driver
    let usb_bus = UsbBusAllocator::new(hal::usb::UsbBus::new(
        pac.USBCTRL_REGS,
        pac.USBCTRL_DPRAM,
        clocks.usb_clock,
        true,
        &mut pac.RESETS,
    ));

    // Set up the USB Communications Class Device driver
    let mut serial = SerialPort::new(&usb_bus);

    // Create a USB device with a fake VID and PID
    let mut usb_dev = UsbDeviceBuilder::new(&usb_bus, UsbVidPid(0x16c0, 0x27dd))
        .strings(&[StringDescriptors::default()
            .manufacturer("Fake company")
            .product("Serial port")
            .serial_number("TEST")])
        .unwrap()
        .device_class(2) // from: https://www.usb.org/defined-class-codes
        .build();

    // Set up the GPIO pins
    let sio = hal::Sio::new(pac.SIO);
    let pins = rp_pico::Pins::new(
        pac.IO_BANK0,
        pac.PADS_BANK0,
        sio.gpio_bank0,
        &mut pac.RESETS,
    );

    // Turn on the onboard LED to indicate the Pico is running
    let mut led_pin = pins.led.into_push_pull_output();
    pin_on(&mut led_pin).unwrap();

    #[cfg(feature = "probe")]
    debug!("LED turned on");

    // Initialize the ADC
    let mut adc = Adc::new(pac.ADC, &mut pac.RESETS);

    // Configure ADC pins for potentiometers
    // Using 3 available ADC inputs: ADC0 (GPIO26), ADC1 (GPIO27), ADC2 (GPIO28)
    let mut adc_pin_0 = AdcPin::new(pins.gpio26.into_floating_input()).unwrap();
    let mut adc_pin_1 = AdcPin::new(pins.gpio27.into_floating_input()).unwrap();
    let mut adc_pin_2 = AdcPin::new(pins.gpio28.into_floating_input()).unwrap();

    // Don't use cortex_m delay - it blocks USB!

    let mut said_hello = false;
    let mut counter = 0u32;
    loop {
        // A welcome message at the beginning
        if !said_hello {
            said_hello = true;
            let _ = serial.write(b"Hello, World!\r\n");
        }

        // Check for new data
        if usb_dev.poll(&mut [&mut serial]) {
            let mut buf = [0u8; 64];
            match serial.read(&mut buf) {
                Err(_e) => {
                    // Do nothing
                }
                Ok(0) => {
                    // Do nothing
                }
                Ok(count) => {
                    // Convert to upper case
                    buf.iter_mut().take(count).for_each(|b| {
                        b.make_ascii_uppercase();
                    });
                    // Send back to the host
                    let mut wr_ptr = &buf[..count];
                    while !wr_ptr.is_empty() {
                        match serial.write(wr_ptr) {
                            Ok(len) => wr_ptr = &wr_ptr[len..],
                            // On error, just drop unwritten data.
                            Err(_) => break,
                        };
                    }
                }
            }
        }

        // Send JSON data periodically (roughly every 10000 polls for ~50ms at USB polling rate)
        if counter.is_multiple_of(10000) {
            // Read potentiometers
            let pot1_raw: u16 = block!(adc.read(&mut adc_pin_0)).unwrap_or(0);
            let pot2_raw: u16 = block!(adc.read(&mut adc_pin_1)).unwrap_or(0);
            let pot3_raw: u16 = block!(adc.read(&mut adc_pin_2)).unwrap_or(0);

            // Create JSON manually to avoid heap allocation
            let mut json: String<64> = String::new();
            let _ = writeln!(
                &mut json,
                "{{\"pot1\":{},\"pot2\":{},\"pot3\":{}}}",
                pot1_raw, pot2_raw, pot3_raw
            );
            let _ = serial.write(json.as_bytes());
        }

        counter = counter.wrapping_add(1);
        // No delay - just keep polling USB!
    }
}
