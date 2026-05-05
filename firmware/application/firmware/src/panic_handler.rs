use core::{
    fmt::{self, Write},
    panic::PanicInfo,
};

use portable_atomic::{AtomicBool, Ordering};

use cortex_m::{asm, interrupt};

use embassy_stm32::{
    mode::Blocking,
    peripherals,
    usart::{Config, UartTx},
};

static PANICKING: AtomicBool = AtomicBool::new(false);

struct PanicUart {
    tx: UartTx<'static, Blocking>,
}

impl PanicUart {
    /// # Safety
    ///
    /// Only call this from the panic path after interrupts are disabled,
    /// and only if the program will never return to normal execution.
    unsafe fn steal() -> Option<Self> {
        let mut config = Config::default();
        config.baudrate = 115_200;

        // Adjust these to your board:
        //   USART1 + PA9 is a common USART1_TX mapping, but not universal.
        let tx = UartTx::new_blocking(
            unsafe { peripherals::USART2::steal() },
            unsafe { peripherals::PA2::steal() },
            config,
        )
        .ok()?;

        Some(Self { tx })
    }

    fn flush(&mut self) {
        let _ = self.tx.blocking_flush();
    }
}

impl Write for PanicUart {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for &b in s.as_bytes() {
            if b == b'\n' {
                let _ = self.tx.blocking_write(b"\r");
            }
            let _ = self.tx.blocking_write(core::slice::from_ref(&b));
        }
        Ok(())
    }
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    interrupt::disable();

    // Avoid recursive panic printing.
    if !PANICKING.swap(true, Ordering::Relaxed) {
        if let Some(mut uart) = unsafe { PanicUart::steal() } {
            let _ = writeln!(uart, "");
            let _ = writeln!(uart, "{}", info);
            let _ = writeln!(uart, "");
            uart.flush();
        }
    }

    loop {
        asm::udf();
    }
}
