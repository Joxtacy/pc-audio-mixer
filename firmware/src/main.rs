//! PC Audio Mixer - Reads 8 slide potentiometers and sends values over USB
//!
//! This application reads analog values from 8 potentiometers connected via MCP3008 ADC
//! and transmits their values to a PC over USB CDC (serial communication).
//!
//! MCP3008 Wiring:
//! - VDD/VREF → 3.3V
//! - AGND/DGND → GND
//! - CLK → GPIO18 (SPI0 SCK)
//! - DOUT → GPIO16 (SPI0 MISO)
//! - DIN → GPIO19 (SPI0 MOSI)
//! - CS → GPIO17 (SPI0 CS)
//! - CH0-CH7 → Potentiometer wipers (all 8 channels used)

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
    clocks::{init_clocks_and_plls, Clock},
    fugit::RateExtU32,
    gpio::FunctionSpi,
    spi::Spi,
};

// Import embedded-hal v0.2 traits
use embedded_hal::digital::v2::OutputPin;
use embedded_hal::spi::FullDuplex;

use usb_device::device::StringDescriptors;
use usb_device::{class_prelude::*, prelude::*};
use usbd_serial::SerialPort;

use core::fmt::Write;
use heapless::String;

// MCP3008 ADC driver
struct Mcp3008<SPI, CS> {
    spi: SPI,
    cs_pin: CS,
}

impl<SPI, CS> Mcp3008<SPI, CS>
where
    SPI: FullDuplex<u8>,
    CS: OutputPin,
{
    fn new(spi: SPI, cs_pin: CS) -> Self {
        Self { spi, cs_pin }
    }

    fn read_channel(&mut self, channel: u8) -> Result<u16, ()> {
        if channel > 7 {
            return Err(());
        }

        // MCP3008 expects:
        // Byte 0: [x x x x x START SGL/DIFF D2]
        // Byte 1: [D1 D0 x x x x x x]
        // Byte 2: [x x x x x x x x]
        // Where START=1, SGL/DIFF=1 for single-ended, D2-D0 = channel

        // Build the command bytes correctly
        let start_bit = 0x01;
        let single_ended = 0x80; // SGL/DIFF = 1 for single-ended

        // First byte: START bit (bit 0)
        let tx_buf = [
            start_bit,
            single_ended | (channel << 4), // SGL/DIFF + channel high bits
            0x00,
        ];
        let mut rx_buf = [0u8; 3];

        self.cs_pin.set_low().ok();

        // Transfer data
        for i in 0..3 {
            // Send byte and wait for response
            if nb::block!(self.spi.send(tx_buf[i])).is_err() {
                self.cs_pin.set_high().ok();
                return Err(());
            }
            match nb::block!(self.spi.read()) {
                Ok(data) => rx_buf[i] = data,
                Err(_) => {
                    self.cs_pin.set_high().ok();
                    return Err(());
                }
            }
        }

        self.cs_pin.set_high().ok();

        // Extract 10-bit result from received bytes
        let result = ((rx_buf[1] as u16 & 0x03) << 8) | (rx_buf[2] as u16);
        Ok(result)
    }
}

#[entry]
fn main() -> ! {
    #[cfg(feature = "probe")]
    info!("PC Audio Mixer starting...");

    // Take ownership of the device peripherals
    let mut pac = pac::Peripherals::take().unwrap();
    let _core = pac::CorePeripherals::take().unwrap();

    // Set up the watchdog driver - needed by the clock setup code
    let mut watchdog = hal::Watchdog::new(pac.WATCHDOG);

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
    let _ = led_pin.set_high();

    #[cfg(feature = "probe")]
    debug!("LED turned on");

    // Set up SPI for MCP3008
    let spi_pins = (
        pins.gpio19.into_function::<FunctionSpi>(), // MOSI
        pins.gpio16.into_function::<FunctionSpi>(), // MISO
        pins.gpio18.into_function::<FunctionSpi>(), // SCK
    );

    let spi = Spi::<_, _, _, 8>::new(pac.SPI0, spi_pins).init(
        &mut pac.RESETS,
        clocks.peripheral_clock.freq(),
        1_000_000u32.Hz(), // 1 MHz SPI clock (safe for 3.3V operation)
        embedded_hal::spi::MODE_0,
    );

    let cs_pin = pins.gpio17.into_push_pull_output();
    let mut mcp3008 = Mcp3008::new(spi, cs_pin);

    #[cfg(feature = "probe")]
    info!("MCP3008 initialized, starting main loop...");

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
            // Read potentiometers from MCP3008 channels 0, 1, 2
            let pot1_raw = mcp3008.read_channel(0).unwrap_or(0);
            let pot2_raw = mcp3008.read_channel(1).unwrap_or(0);
            let pot3_raw = mcp3008.read_channel(2).unwrap_or(0);
            let pot4_raw = mcp3008.read_channel(3).unwrap_or(0);
            let pot5_raw = mcp3008.read_channel(4).unwrap_or(0);
            let pot6_raw = mcp3008.read_channel(5).unwrap_or(0);
            let pot7_raw = mcp3008.read_channel(6).unwrap_or(0);
            let pot8_raw = mcp3008.read_channel(7).unwrap_or(0);

            #[cfg(feature = "probe")]
            info!(
                "\npot1: {}\npot2: {}\npot3: {}\npot4: {}\npot5: {}\npot6: {}\npot7: {}\npot8: {}",
                pot1_raw, pot2_raw, pot3_raw, pot4_raw, pot5_raw, pot6_raw, pot7_raw, pot8_raw
            );

            // Create JSON manually to avoid heap allocation
            let mut json: String<256> = String::new();
            let _ = writeln!(
                &mut json,
                "{{\"pot1\":{},\"pot2\":{},\"pot3\":{},\"pot4\":{},\"pot5\":{},\"pot6\":{},\"pot7\":{},\"pot8\":{}}}",
                pot1_raw, pot2_raw, pot3_raw, pot4_raw, pot5_raw, pot6_raw, pot7_raw, pot8_raw
            );
            let _ = serial.write(json.as_bytes());
        }

        counter = counter.wrapping_add(1);
    }
}
