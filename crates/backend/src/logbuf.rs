//! In-memory console-log capture: a capped ring buffer plus a broadcast channel
//! so the dashboard can stream server logs live over SSE. A `tracing` layer
//! feeds every event into the buffer.

use std::collections::VecDeque;
use std::fmt::Debug;
use std::sync::{Arc, Mutex};
use tokio::sync::broadcast;
use tracing::field::{Field, Visit};
use tracing_subscriber::layer::{Context, Layer};

const CAP: usize = 1000;

#[derive(Clone)]
pub enum LogMsg {
    Line(String),
    Clear,
}

pub struct LogBuffer {
    buf: Mutex<VecDeque<String>>,
    tx: broadcast::Sender<LogMsg>,
}

impl LogBuffer {
    pub fn new() -> Self {
        let (tx, _) = broadcast::channel(256);
        Self {
            buf: Mutex::new(VecDeque::with_capacity(CAP)),
            tx,
        }
    }

    pub fn push(&self, line: String) {
        {
            let mut b = self.buf.lock().unwrap();
            if b.len() >= CAP {
                b.pop_front();
            }
            b.push_back(line.clone());
        }
        let _ = self.tx.send(LogMsg::Line(line));
    }

    pub fn snapshot(&self) -> Vec<String> {
        self.buf.lock().unwrap().iter().cloned().collect()
    }

    pub fn clear(&self) {
        self.buf.lock().unwrap().clear();
        let _ = self.tx.send(LogMsg::Clear);
    }

    pub fn subscribe(&self) -> broadcast::Receiver<LogMsg> {
        self.tx.subscribe()
    }
}

/// A `tracing` layer that formats each event as `[LEVEL] message` and appends it
/// to the shared [`LogBuffer`].
pub struct BufferLayer {
    pub buf: Arc<LogBuffer>,
}

struct MsgVisitor {
    msg: String,
}

impl Visit for MsgVisitor {
    fn record_debug(&mut self, field: &Field, value: &dyn Debug) {
        if field.name() == "message" {
            self.msg = format!("{value:?}");
        }
    }
}

impl<S: tracing::Subscriber> Layer<S> for BufferLayer {
    fn on_event(&self, event: &tracing::Event<'_>, _ctx: Context<'_, S>) {
        let level = *event.metadata().level();
        let mut v = MsgVisitor { msg: String::new() };
        event.record(&mut v);
        self.buf.push(format!("[{level}] {}", v.msg));
    }
}
