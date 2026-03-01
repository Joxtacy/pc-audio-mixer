//! PC Audio Mixer - Reads up to 8 potentiometers via MCP3008 and sends values over USB
//!
//! This application reads analog values from potentiometers connected to an MCP3008 ADC
//! and transmits their values to a PC over USB CDC (serial communication).
//!
//! MCP3008 Wiring:
//! - VDD/VREF → 3.3V
//! - AGND/DGND → GND
//! - CLK → GPIO18 (SPI0 SCK)
//! - DOUT → GPIO16 (SPI0 MISO)
//! - DIN → GPIO19 (SPI0 MOSI)
//! - CS → GPIO17 (SPI0 CS)
//! - CH0-CH7 → Potentiometer wipers

#![no_std]
#![no_main]

use bsp::entry;
use defmt::*;
use defmt_rtt as _;
use panic_probe as _;

use rp_pico as bsp;

use bsp::hal::{
    clocks::{init_clocks_and_plls, Clock},
    gpio::{FunctionSpi, Pin},
    pac,
    sio::Sio,
    spi::{Enabled, Spi, SpiDevice},
    usb::UsbBus,
    watchdog::Watchdog,
};

use embedded_hal::digital::OutputPin;
use embedded_hal::spi::SpiBus;
use serde::Serialize;
use usb_device::{class_prelude::*, prelude::*};
use usbd_serial::{SerialPort, USB_CLASS_CDC};

// Structure to hold potentiometer readings
#[derive(Serialize)]
struct PotentiometerData {
    pot1: u16,
    pot2: u16,
    pot3: u16,
    pot4: u16,
    pot5: u16,
    pot6: u16,
    // pot7: u16, // Uncomment for 7th channel
    // pot8: u16, // Uncomment for 8th channel
}

type SpiType = Spi<
    Enabled,
    pac::SPI0,
    (
        Pin<bsp::hal::gpio::bank0::Gpio16, FunctionSpi, bsp::hal::gpio::PullDown>,
        Pin<bsp::hal::gpio::bank0::Gpio19, FunctionSpi, bsp::hal::gpio::PullDown>,
        Pin<bsp::hal::gpio::bank0::Gpio18, FunctionSpi, bsp::hal::gpio::PullDown>,
    ),
>;

struct Mcp3008 {
    spi: SpiType,
    cs_pin: Pin<
        bsp::hal::gpio::bank0::Gpio17,
        bsp::hal::gpio::Output<bsp::hal::gpio::PushPull>,
        bsp::hal::gpio::PullDown,
    >,
}

impl Mcp3008 {
    fn new(
        spi: SpiType,
        cs_pin: Pin<
            bsp::hal::gpio::bank0::Gpio17,
            bsp::hal::gpio::Output<bsp::hal::gpio::PushPull>,
            bsp::hal::gpio::PullDown,
        >,
    ) -> Self {
        Self { spi, cs_pin }
    }

    fn read_channel(&mut self, channel: u8) -> Result<u16, ()> {
        if channel > 7 {
            return Err(());
        }

        // MCP3008 command: start bit + single-ended + channel selection
        let command = 0x01; // Start bit
        let command = (command << 4) | 0x08; // Single-ended mode
        let command = (command << 3) | channel; // Channel selection

        // SPI transaction: send 3 bytes, get 3 bytes back
        let mut tx_buf = [command, 0x00, 0x00];
        let mut rx_buf = [0u8; 3];

        self.cs_pin.set_low().ok();

        // Transfer data
        for i in 0..3 {
            match self.spi.transfer(&mut [tx_buf[i]]) {
                Ok(received) => rx_buf[i] = received[0],
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
    info!("PC Audio Mixer with MCP3008 starting...");

    let mut pac = pac::Peripherals::take().unwrap();
    let core = pac::CorePeripherals::take().unwrap();
    let mut watchdog = Watchdog::new(pac.WATCHDOG);

    let clocks = init_clocks_and_plls(
        12_000_000u32,
        pac.XOSC,
        pac.CLOCKS,
        pac.PLL_SYS,
        pac.PLL_USB,
        &mut pac.RESETS,
        &mut watchdog,
    )
    .ok()
    .unwrap();

    // Set up USB
    let usb_bus = UsbBusAllocator::new(UsbBus::new(
        pac.USBCTRL_REGS,
        pac.USBCTRL_DPRAM,
        clocks.usb_clock,
        true,
        &mut pac.RESETS,
    ));

    let mut serial = SerialPort::new(&usb_bus);
    let mut usb_dev = UsbDeviceBuilder::new(&usb_bus, UsbVidPid(0x16c0, 0x27dd))
        .device_class(USB_CLASS_CDC)
        .build();

    // Set up GPIO pins
    let sio = Sio::new(pac.SIO);
    let pins = bsp::Pins::new(
        pac.IO_BANK0,
        pac.PADS_BANK0,
        sio.gpio_bank0,
        &mut pac.RESETS,
    );

    // Set up SPI for MCP3008
    let spi_pins = (
        pins.gpio16.into_function::<FunctionSpi>(), // MISO
        pins.gpio19.into_function::<FunctionSpi>(), // MOSI
        pins.gpio18.into_function::<FunctionSpi>(), // SCK
    );

    let spi = Spi::<_, _, _, 8>::new(pac.SPI0, spi_pins).init(
        &mut pac.RESETS,
        clocks.peripheral_clock.freq(),
        1_000_000u32.Hz(), // 1 MHz SPI clock
        embedded_hal::spi::MODE_0,
    );

    let cs_pin = pins.gpio17.into_push_pull_output();
    let mut mcp3008 = Mcp3008::new(spi, cs_pin);

    let mut delay = cortex_m::delay::Delay::new(core.SYST, clocks.system_clock.freq().to_Hz());

    info!("Setup complete, starting main loop...");

    loop {
        if usb_dev.poll(&mut [&mut serial]) {
            // Handle USB events
        }

        // Read all 6 potentiometer channels
        let pot1 = mcp3008.read_channel(0).unwrap_or(0);
        let pot2 = mcp3008.read_channel(1).unwrap_or(0);
        let pot3 = mcp3008.read_channel(2).unwrap_or(0);
        let pot4 = mcp3008.read_channel(3).unwrap_or(0);
        let pot5 = mcp3008.read_channel(4).unwrap_or(0);
        let pot6 = mcp3008.read_channel(5).unwrap_or(0);

        let pot_data = PotentiometerData {
            pot1,
            pot2,
            pot3,
            pot4,
            pot5,
            pot6,
        };

        // Send JSON data over USB
        if let Ok(json_string) = serde_json_core::to_string::<_, 256>(&pot_data) {
            let mut full_message = json_string;
            full_message.push('\n').ok();
            let _ = serial.write(full_message.as_bytes());
            info!("Sent: {}", full_message.as_str());
        }

        delay.delay_ms(50); // 20Hz update rate
    }
}
