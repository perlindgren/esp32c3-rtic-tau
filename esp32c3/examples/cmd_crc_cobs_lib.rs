//! cmd
//!
//! Run on target: `cd esp32c3`
//!
//! cargo embed --example cmd_crc_cobs_lib --release
//!
//! Run on host: `cd host`
//! cargo run --example cmd_crc_cobs_lib
//!
//! Demonstrates ssmarshal + serde + crc + cobs
//!
#![no_std]
#![no_main]
#![feature(type_alias_impl_trait)]

use panic_rtt_target as _;

// bring in panic handler
use panic_rtt_target as _;

#[rtic::app(device = esp32c3, dispatchers = [FROM_CPU_INTR0, FROM_CPU_INTR1])]
mod app {
    use core::fmt::Write;
    use esp32c3_hal::{
        clock::ClockControl,
        peripherals::{Peripherals, TIMG0, UART0},
        prelude::*,
        timer::{Timer, Timer0, TimerGroup},
        uart::{
            config::{Config, DataBits, Parity, StopBits},
            TxRxPins,
        },
        Uart, IO,
    };
    use nb::block;
    use rtic_sync::{channel::*, make_channel};
    use rtt_target::{rprint, rprintln, rtt_init_print};

    #[shared]
    struct Shared {
        uart0: Uart<'static, UART0>,
    }

    #[local]
    struct Local {
        timer0: Timer<Timer0<TIMG0>>,
    }

    const CAPACITY: usize = 1;
    #[init]
    fn init(_: init::Context) -> (Shared, Local) {
        rtt_init_print!();
        rprintln!("uart_echo");
        let (s, r) = make_channel!(u32, CAPACITY);

        let peripherals = Peripherals::take();
        let mut system = peripherals.SYSTEM.split();
        let clocks = ClockControl::max(system.clock_control).freeze();

        let timer_group0 = TimerGroup::new(
            peripherals.TIMG0,
            &clocks,
            &mut system.peripheral_clock_control,
        );
        let mut timer0 = timer_group0.timer0;

        let config = Config {
            baudrate: 115200,
            data_bits: DataBits::DataBits8,
            parity: Parity::ParityNone,
            stop_bits: StopBits::STOP1,
        };

        let io = IO::new(peripherals.GPIO, peripherals.IO_MUX);
        let pins = TxRxPins::new_tx_rx(
            io.pins.gpio0.into_push_pull_output(),
            io.pins.gpio1.into_floating_input(),
        );

        let mut uart0 = Uart::new_with_config(
            peripherals.UART0,
            config,
            Some(pins),
            &clocks,
            &mut system.peripheral_clock_control,
        );

        // This is stupid!
        // TODO, can we have interrupts after timeout even if threshold > 1?
        uart0.set_rx_fifo_full_threshold(1).unwrap();
        uart0.listen_rx_fifo_full();

        timer0.start(1u64.secs());

        lowprio::spawn(r).unwrap();
        sender::spawn(s).unwrap();

        (Shared { uart0 }, Local { timer0 })
    }

    #[idle(local = [timer0], shared = [uart0])]
    fn idle(mut cx: idle::Context) -> ! {
        loop {
            cx.shared.uart0.lock(|uart0| {
                writeln!(uart0, "Hello to Finland from Esp32C3!").unwrap();
            });
            block!(cx.local.timer0.wait()).unwrap();
        }
    }

    #[task(binds = UART0, priority=2, shared=[uart0])]
    fn uart0(mut cx: uart0::Context) {
        rprint!("Interrupt Received: ");
        cx.shared.uart0.lock(|uart0| {
            while let nb::Result::Ok(c) = uart0.read() {
                writeln!(uart0, "Read byte: {:02x}", c).unwrap();
                rprint!("{}", c as char);
            }
            uart0.reset_rx_fifo_full_interrupt()
        });
    }

    #[task(priority = 1)]
    async fn sender(mut cx: sender::Context, mut sender: Sender<'static, u32, CAPACITY>) {
        rprintln!("sender started");
        let f = sender.send(32);
        rprintln!("sender_sent");
        let r = f.await;

        rprintln!("sender r {:?}", r);
    }

    #[task(priority = 2, shared=[uart0])]
    async fn lowprio(mut cx: lowprio::Context, mut receiver: Receiver<'static, u32, CAPACITY>) {
        rprintln!("LowPrio started");

        while let Ok(val) = receiver.recv().await {
            rprintln!("Receiver got: {}", val);
        }

        // cx.shared.uart0.lock(|uart0| {
        //     while let nb::Result::Ok(c) = uart0.read() {
        //         writeln!(uart0, "Read byte: {:02x}", c).unwrap();
        //         rprint!("{}", c as char);
        //     }
        //     uart0.reset_rx_fifo_full_interrupt()
        // });
    }
}

// #[rtic::app(device = atsamx7x_hal::pac, peripherals = true, dispatchers = [IXC])]
// mod app {
//     // Backend dependencies
//     use atsamx7x_hal as hal;
//     use hal::clocks::*;
//     use hal::efc::*;
//     use hal::ehal::serial::{Read, Write};
//     use hal::fugit::RateExtU32;
//     use hal::generics::events::EventHandler;
//     use hal::pio::*;
//     use hal::serial::uart::UartConfiguration;
//     use hal::serial::{
//         usart::{Event, Rx, Tx, Usart, Usart1},
//         ExtBpsU32,
//     };
//     use rtt_target::{rprint, rprintln, rtt_init_print};

//     // Application dependencies
//     use core::mem::size_of;
//     use corncobs::{max_encoded_len, ZERO};
//     use master_and_servant::{deserialize_crc_cobs, serialize_crc_cobs, Command, Response};
//     use nb::block;

//     const IN_SIZE: usize = max_encoded_len(size_of::<Command>() + size_of::<u32>());
//     const OUT_SIZE: usize = max_encoded_len(size_of::<Response>() + size_of::<u32>());

//     #[shared]
//     struct Shared {}

//     #[local]
//     struct Local {
//         tx: Tx<Usart1>,
//         rx: Rx<Usart1>,
//         usart: Usart<Usart1>,
//     }

//     #[init()]
//     fn init(ctx: init::Context) -> (Shared, Local, init::Monotonics) {
//         rtt_init_print!();
//         rprintln!("init");
//         let pac = ctx.device;

//         let clocks = Tokens::new((pac.PMC, pac.SUPC, pac.UTMI), &pac.WDT.into());
//         let slck = clocks.slck.configure_external_normal();
//         let mainck = clocks.mainck.configure_external_normal(12.MHz()).unwrap();
//         let mut efc = Efc::new(pac.EFC, VddioLevel::V3);
//         let (_hclk, mut mck) = HostClockController::new(clocks.hclk, clocks.mck)
//             .configure(
//                 &mainck,
//                 &mut efc,
//                 HostClockConfig {
//                     pres: HccPrescaler::Div1,
//                     div: MckDivider::Div1,
//                 },
//             )
//             .unwrap();
//         let _pck: Pck<Pck4> = clocks.pcks.pck4.configure(&mainck, 1).unwrap();

//         let banka = BankA::new(pac.PIOA, &mut mck, &slck, BankConfiguration::default());
//         let bankb = BankB::new(pac.PIOB, &mut mck, &slck, BankConfiguration::default());

//         // usart1
//         let miso = banka.pa21.into_peripheral(); // RXD1
//         let mosi = bankb.pb4.into_peripheral(); // TXD1
//         let clk = banka.pa23.into_peripheral(); // CKL1
//         let nss = banka.pa24.into_peripheral(); // RTS1?

//         // Create the top-level USART abstraction
//         let (handles, mut usart) = Usart::new_usart1(pac.USART1, (mosi, miso, clk, nss), &mut mck);

//         // consume the usart token and turn it into a uart
//         let uart = handles
//             .uart
//             .configure(&usart, &mck, UartConfiguration::default(9600.bps()))
//             .unwrap();

//         // Listen to an interrupt event.
//         // usart.listen_slice(&[Event::RxReady, Event::TxReady]); to listen also for TxReady
//         usart.listen_slice(&[Event::RxReady]);

//         usart.enter_mode(&uart);
//         let (tx, rx) = uart.split();

//         (Shared {}, Local { tx, rx, usart }, init::Monotonics())
//     }

//     #[task(binds=USART1, local = [rx, usart], priority = 2)]
//     fn usart(ctx: usart::Context) {
//         use hal::serial::usart::Event::*;

//         let usart::LocalResources { rx, usart } = ctx.local;
//         for event in usart.events() {
//             match event {
//                 RxReady => {
//                     let data = rx.read().unwrap();
//                     let _ = lowprio::spawn(data);
//                 }
//                 TxReady => {
//                     // uart.write(b'r');
//                 }
//                 TxEmpty => {
//                     // uart.write(b'e');
//                 }
//                 _ => {
//                     rprintln!("event {:?}", event);
//                     rx.clear_errors();
//                 }
//             }
//         }
//     }

//     #[task(
//         priority = 1,
//         capacity = 100,
//         local = [
//             tx,
//             // locally initialized resources
//             index: usize = 0,
//             in_buf: [u8; IN_SIZE] = [0u8; IN_SIZE],
//             out_buf: [u8; OUT_SIZE] = [0u8; OUT_SIZE]
//         ]
//     )]
//     fn lowprio(ctx: lowprio::Context, data: u8) {
//         let lowprio::LocalResources {
//             tx,
//             index,
//             in_buf,
//             out_buf,
//         } = ctx.local;
//         rprint!("r{} {}", data, index);
//         in_buf[*index] = data;

//         // ensure index in range
//         if *index < IN_SIZE - 1 {
//             *index += 1;
//         }

//         // end of cobs frame
//         if data == ZERO {
//             rprintln!("\n-- cobs packet received {:?} --", &in_buf[0..*index]);
//             *index = 0;

//             match deserialize_crc_cobs::<Command>(in_buf) {
//                 Ok(cmd) => {
//                     rprintln!("cmd {:?}", cmd);
//                     let response = match cmd {
//                         Command::Set(_id, _par, _dev) => Response::SetOk,
//                         Command::Get(id, par, dev) => Response::Data(id, par, 42, dev),
//                     };
//                     rprintln!("response {:?}", response);
//                     let to_write = serialize_crc_cobs(&response, out_buf);
//                     for byte in to_write {
//                         block!(tx.write(*byte)).unwrap();
//                     }
//                 }

//                 Err(err) => {
//                     rprintln!("ssmarshal err {:?}", err);
//                 }
//             }
//         }
//     }
// }
