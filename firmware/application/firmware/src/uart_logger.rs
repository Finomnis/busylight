use core::fmt::{self, Write};

use bbqueue::prod_cons::stream::{StreamConsumer, StreamProducer};
use embassy_stm32::{mode::Async, usart::UartTx};
use git_version::git_version;
use log::{Level, Metadata, Record};

use crate::Mono;
use rtic_monotonics::Monotonic;

const LOG_QUEUE_SIZE: usize = 8192;

type LogQueue =
    bbqueue::nicknames::Memphis<LOG_QUEUE_SIZE, bbqueue::traits::notifier::maitake::MaiNotSpsc>;

static QUEUE: LogQueue = LogQueue::new();
static LOGGER: BBQueueLogger = BBQueueLogger::new(Level::Info);

struct StreamWriter<'a>(&'a StreamProducer<&'static LogQueue>);

impl Write for StreamWriter<'_> {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        let num_newlines = s.chars().filter(|&c| c == '\n').count();

        let grant_len = num_newlines + s.len();
        let mut grant = self.0.grant_exact(grant_len).map_err(|_| fmt::Error)?;

        let buf = grant.as_mut();
        let mut i = 0;
        for &b in s.as_bytes() {
            if b == b'\n' {
                buf[i] = b'\r';
                i += 1;
            }
            buf[i] = b;
            i += 1;
        }

        grant.commit(grant_len);

        Ok(())
    }
}

struct BBQueueLogger {
    level: Level,
    queue: StreamProducer<&'static LogQueue>,
}

impl BBQueueLogger {
    pub const fn new(level: Level) -> Self {
        Self {
            level,
            queue: QUEUE.stream_producer(),
        }
    }
}

impl log::Log for BBQueueLogger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= self.level
    }

    fn log(&self, record: &Record) {
        if self.enabled(record.metadata()) {
            let _ = writeln!(
                StreamWriter(&self.queue),
                "{:>11.06}: {:>5} - {}",
                Mono::now().duration_since_epoch().as_secs_f32(),
                record.level(),
                record.args()
            );
        }
    }

    fn flush(&self) {}
}

pub struct UartLogHandler {
    uart: UartTx<'static, Async>,
    queue: StreamConsumer<&'static LogQueue>,
}

impl UartLogHandler {
    pub async fn run(&mut self) {
        let _ = self
            .uart
            .write(b"\r\n\r\n====================================================\r\n Busylight (git: ")
            .await;
        let _ = self
            .uart
            .write(git_version!(args = ["--tags", "--always", "--dirty"]).as_bytes())
            .await;
        let _ = self
            .uart
            .write(b")\r\n====================================================\r\n")
            .await;
        loop {
            let grant = self.queue.wait_read().await;
            let len = grant.len();
            let _ = self.uart.write(&grant).await;
            grant.release(len);
        }
    }
}

pub fn init(uart: UartTx<'static, Async>) -> UartLogHandler {
    unsafe {
        log::set_logger_racy(&LOGGER).unwrap();
        log::set_max_level_racy(log::LevelFilter::Info);
    }

    UartLogHandler {
        uart,
        queue: QUEUE.stream_consumer(),
    }
}
