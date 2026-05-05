use core::{
    cell::RefCell,
    fmt::{self, Write},
};

use bbqueue::prod_cons::stream::{StreamConsumer, StreamProducer};
use embassy_stm32::{mode::Async, usart::UartTx};
use log::{Level, Metadata, Record};

const LOG_QUEUE_SIZE: usize = 8192;

type LogQueue =
    bbqueue::nicknames::Memphis<LOG_QUEUE_SIZE, bbqueue::traits::notifier::maitake::MaiNotSpsc>;

static QUEUE: LogQueue = LogQueue::new();
static LOGGER: BBQueueLogger = BBQueueLogger::new(Level::Info);

struct StreamWriter {
    queue: StreamProducer<&'static LogQueue>,
}

impl Write for StreamWriter {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        let mut grant = self.queue.grant_exact(s.len()).map_err(|_| fmt::Error)?;
        grant.copy_from_slice(s.as_bytes());
        grant.commit(s.len());

        Ok(())
    }
}

struct BBQueueLogger {
    level: Level,
    queue: critical_section::Mutex<RefCell<StreamWriter>>,
}

impl BBQueueLogger {
    pub const fn new(level: Level) -> Self {
        Self {
            level,
            queue: critical_section::Mutex::new(RefCell::new(StreamWriter {
                queue: QUEUE.stream_producer(),
            })),
        }
    }
}

impl log::Log for BBQueueLogger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= self.level
    }

    fn log(&self, record: &Record) {
        if self.enabled(record.metadata()) {
            critical_section::with(|cs| {
                let mut queue = self.queue.borrow(cs).borrow_mut();
                let _ = writeln!(queue, "{} - {}", record.level(), record.args());
            });
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
        loop {
            let grant = self.queue.wait_read().await;
            let _ = self.uart.write(&grant).await;
        }
    }
}

pub fn init(uart: UartTx<'static, Async>) -> UartLogHandler {
    unsafe { log::set_logger_racy(&LOGGER).unwrap() };
    UartLogHandler {
        uart,
        queue: QUEUE.stream_consumer(),
    }
}
