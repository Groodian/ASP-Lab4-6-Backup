use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

pub struct ServerStop {
    server_thread_stops: Vec<ServerThreadStop>,
}

pub struct ServerThreadStop {
    should_stop: Arc<AtomicBool>,
}

impl ServerStop {
    pub fn new(server_thread_stops: Vec<ServerThreadStop>) -> Self {
        Self {
            server_thread_stops,
        }
    }

    pub fn stop(&self) {
        for server_stop_thread in &self.server_thread_stops {
            server_stop_thread.stop();
        }
    }
}

impl Clone for ServerStop {
    fn clone(&self) -> Self {
        Self {
            server_thread_stops: self.server_thread_stops.clone(),
        }
    }
}

impl ServerThreadStop {
    pub fn new() -> Self {
        Self {
            should_stop: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn stop(&self) {
        self.should_stop.store(true, Ordering::SeqCst);
    }

    pub fn should_stop(&self) -> bool {
        return self.should_stop.load(Ordering::SeqCst);
    }
}

impl Clone for ServerThreadStop {
    fn clone(&self) -> Self {
        Self {
            should_stop: Arc::clone(&self.should_stop),
        }
    }
}
