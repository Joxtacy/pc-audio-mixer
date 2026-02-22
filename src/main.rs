//! Toggles the LED on a Pico board with a button
//!
//! This will toggle an LED attached to GP25 when a button on GP2 is pressed.
#![no_std]
#![no_main]

use bsp::entry;
use defmt::*;
use defmt_rtt as _;
use embedded_hal::digital::{InputPin, OutputPin};
use panic_probe as _;

// Provide an alias for our BSP so we can switch targets quickly.
// Uncomment the BSP you included in Cargo.toml, the rest of the code does not need to change.
use rp_pico as bsp;
// use sparkfun_pro_micro_rp2040 as bsp;

use bsp::hal::{
    clocks::{init_clocks_and_plls, Clock},
    pac,
    sio::Sio,
    watchdog::Watchdog,
};

#[entry]
fn main() -> ! {
    info!("Program start");
    let mut pac = pac::Peripherals::take().unwrap();
    let core = pac::CorePeripherals::take().unwrap();
    let mut watchdog = Watchdog::new(pac.WATCHDOG);
    let sio = Sio::new(pac.SIO);

    // External high-speed crystal on the pico board is 12Mhz
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

    let mut delay = cortex_m::delay::Delay::new(core.SYST, clocks.system_clock.freq().to_Hz());

    let pins = bsp::Pins::new(
        pac.IO_BANK0,
        pac.PADS_BANK0,
        sio.gpio_bank0,
        &mut pac.RESETS,
    );

    // Configure the LED pin (GP25) as a push-pull output
    let mut led_pin = pins.led.into_push_pull_output();

    // Configure GPIO2 as an input with internal pull-up resistor
    // When button is not pressed: reads HIGH (3.3V)
    // When button is pressed (connected to ground): reads LOW (0V)
    let mut button_pin = pins.gpio2.into_pull_up_input();

    // Track the current state of the LED (off initially)
    let mut led_state = false;

    // Store the previous button state to detect state changes
    // Start by reading the current button state
    let mut last_button_state = button_pin.is_high().unwrap();

    loop {
        // Read the current button state
        let current_button_state = button_pin.is_high().unwrap();

        // Detect button press: transition from HIGH (not pressed) to LOW (pressed)
        // This is called "falling edge" detection
        if last_button_state && !current_button_state {
            // Toggle the LED state
            led_state = !led_state;

            // Update the actual LED based on the new state
            if led_state {
                info!("LED on!");
                led_pin.set_high().unwrap(); // Turn LED on
            } else {
                info!("LED off!");
                led_pin.set_low().unwrap(); // Turn LED off
            }
        }

        // Remember the current button state for the next iteration
        last_button_state = current_button_state;

        // Small delay to prevent excessive polling and help with button debouncing
        // 50ms is fast enough for responsive button presses but slow enough to avoid bounce
        delay.delay_ms(50);
    }
}

// End of file
