use std::{
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};

pub struct Monitoring {
    duration: Duration,
    start_time: Instant,
    last_time: Instant,
    stats: Vec<Arc<MonitoringStats>>,
}

pub struct MonitoringStats {
    new_connections: AtomicUsize,
    total_new_connections: AtomicUsize,

    lost_connections: AtomicUsize,
    total_lost_connections: AtomicUsize,

    bytes_read: AtomicUsize,
    total_bytes_read: AtomicUsize,

    bytes_send: AtomicUsize,
    total_bytes_send: AtomicUsize,

    messages_received: AtomicUsize,
    total_messages_received: AtomicUsize,

    messeges_send: AtomicUsize,
    total_messages_send: AtomicUsize,
}

impl Monitoring {
    pub fn new(duration: Duration) -> Self {
        Self {
            duration,
            start_time: Instant::now(),
            last_time: Instant::now(),
            stats: Vec::new(),
        }
    }

    pub fn update(&mut self) {
        let elapsed_time = self.last_time.elapsed();
        let elapsed_time_sec = elapsed_time.as_secs_f32();
        if elapsed_time >= self.duration {
            let elapsed_start_time_sec = self.start_time.elapsed().as_secs_f32();

            // collect stats
            let mut new_connections = 0;
            let mut total_new_connections = 0;

            let mut lost_connections = 0;
            let mut total_lost_connections = 0;

            let mut bytes_read = 0;
            let mut total_bytes_read = 0;

            let mut bytes_send = 0;
            let mut total_bytes_send = 0;

            let mut messages_received = 0;
            let mut total_messages_received = 0;

            let mut messeges_send = 0;
            let mut total_messages_send = 0;

            for stats in &self.stats {
                new_connections += stats.new_connections.load(Ordering::Relaxed);
                total_new_connections += stats.total_new_connections.load(Ordering::Relaxed);

                lost_connections += stats.lost_connections.load(Ordering::Relaxed);
                total_lost_connections += stats.total_lost_connections.load(Ordering::Relaxed);

                bytes_read += stats.bytes_read.load(Ordering::Relaxed);
                total_bytes_read += stats.total_bytes_read.load(Ordering::Relaxed);

                bytes_send += stats.bytes_send.load(Ordering::Relaxed);
                total_bytes_send += stats.total_bytes_send.load(Ordering::Relaxed);

                messages_received += stats.messages_received.load(Ordering::Relaxed);
                total_messages_received += stats.total_messages_received.load(Ordering::Relaxed);

                messeges_send += stats.messeges_send.load(Ordering::Relaxed);
                total_messages_send += stats.total_messages_send.load(Ordering::Relaxed);
            }

            // print stats
            println!(
                "                                                                                         \n\
                ----------------------------------------------------------------------------------------- \n\
                Elapsed time: {:.3?}                                                                      \n\
                Current connections: {}                                                                   \n\
                                                                                                          \n\
                Type                   |       last/s  |       total/s  |      last  |     total |        \n\
                Connections:           | {:w1$.3}  |  {:w1$.3}  |  {:w2$}  |  {:w2$} |                    \n\
                Disconnects:           | {:w1$.3}  |  {:w1$.3}  |  {:w2$}  |  {:w2$} |                    \n\
                Bytes read:            | {:w1$.3}  |  {:w1$.3}  |  {:w2$}  |  {:w2$} |                    \n\
                Bytes send:            | {:w1$.3}  |  {:w1$.3}  |  {:w2$}  |  {:w2$} |                    \n\
                Messages received:     | {:w1$.3}  |  {:w1$.3}  |  {:w2$}  |  {:w2$} |                    \n\
                Messages send:         | {:w1$.3}  |  {:w1$.3}  |  {:w2$}  |  {:w2$} |                    \n\
                ----------------------------------------------------------------------------------------- \n\
                ",
                elapsed_time,
                (total_new_connections - total_lost_connections),
                (new_connections as f32 / elapsed_time_sec),
                (total_new_connections as f32 / elapsed_start_time_sec),
                new_connections,
                total_new_connections,
                (lost_connections as f32 / elapsed_time_sec),
                (total_lost_connections as f32 / elapsed_start_time_sec),
                lost_connections,
                total_lost_connections,
                (bytes_read as f32 / elapsed_time_sec),
                (total_bytes_read as f32 / elapsed_start_time_sec),
                bytes_read,
                total_bytes_read,
                (bytes_send as f32 / elapsed_time_sec),
                (total_bytes_send as f32 / elapsed_start_time_sec),
                bytes_send,
                total_bytes_send,
                (messages_received as f32 / elapsed_time_sec),
                (total_messages_received as f32 / elapsed_start_time_sec),
                messages_received,
                total_messages_received,
                (messeges_send as f32 / elapsed_time_sec),
                (total_messages_send as f32 / elapsed_start_time_sec),
                messeges_send,
                total_messages_send,
                w1 = 12,
                w2 = 8
            );

            // reset stats
            for stats in &self.stats {
                stats.new_connections.store(0, Ordering::SeqCst);
                stats.lost_connections.store(0, Ordering::SeqCst);
                stats.bytes_read.store(0, Ordering::SeqCst);
                stats.bytes_send.store(0, Ordering::SeqCst);
                stats.messages_received.store(0, Ordering::SeqCst);
                stats.messeges_send.store(0, Ordering::SeqCst);
            }
            self.last_time = Instant::now();
        }
    }

    pub fn get_new_stats(&mut self) -> Arc<MonitoringStats> {
        let monitoring_stats = Arc::new(MonitoringStats::new());
        let monitoring_stats_return = Arc::clone(&monitoring_stats);
        self.stats.push(monitoring_stats);
        return monitoring_stats_return;
    }
}

impl MonitoringStats {
    pub fn new() -> Self {
        Self {
            new_connections: AtomicUsize::new(0),
            total_new_connections: AtomicUsize::new(0),

            lost_connections: AtomicUsize::new(0),
            total_lost_connections: AtomicUsize::new(0),

            bytes_read: AtomicUsize::new(0),
            total_bytes_read: AtomicUsize::new(0),

            bytes_send: AtomicUsize::new(0),
            total_bytes_send: AtomicUsize::new(0),

            messages_received: AtomicUsize::new(0),
            total_messages_received: AtomicUsize::new(0),

            messeges_send: AtomicUsize::new(0),
            total_messages_send: AtomicUsize::new(0),
        }
    }

    pub fn new_connection(&self) {
        self.new_connections.fetch_add(1, Ordering::SeqCst);
        self.total_new_connections.fetch_add(1, Ordering::SeqCst);
    }

    pub fn lost_connection(&self) {
        self.lost_connections.fetch_add(1, Ordering::SeqCst);
        self.total_lost_connections.fetch_add(1, Ordering::SeqCst);
    }

    pub fn bytes_read(&self, bytes: usize) {
        self.bytes_read.fetch_add(bytes, Ordering::SeqCst);
        self.total_bytes_read.fetch_add(bytes, Ordering::SeqCst);
    }

    pub fn bytes_send(&self, bytes: usize) {
        self.bytes_send.fetch_add(bytes, Ordering::SeqCst);
        self.total_bytes_send.fetch_add(bytes, Ordering::SeqCst);
    }

    pub fn message_received(&self) {
        self.messages_received.fetch_add(1, Ordering::SeqCst);
        self.total_messages_received.fetch_add(1, Ordering::SeqCst);
    }

    pub fn messege_send(&self) {
        self.messeges_send.fetch_add(1, Ordering::SeqCst);
        self.total_messages_send.fetch_add(1, Ordering::SeqCst);
    }
}
