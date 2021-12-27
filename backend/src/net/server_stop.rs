use std::sync::{Arc, Mutex};

pub struct ServerStop {
    server_thread_stops: Vec<ServerThreadStop>,
}

pub struct ServerThreadStop {
    should_stop: Arc<Mutex<bool>>,
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

impl ServerThreadStop {
    pub fn new() -> Self {
        Self {
            should_stop: Arc::new(Mutex::new(false)),
        }
    }

    pub fn stop(&self) {
        let mut should_stop = self.should_stop.lock().unwrap();
        *should_stop = true;
        drop(should_stop);
    }

    pub fn should_stop(&self) -> bool {
        let should_stop = self.should_stop.lock().unwrap();
        let return_value = *should_stop;
        drop(should_stop);

        return_value
    }
}

impl Clone for ServerThreadStop {
    fn clone(&self) -> Self {
        Self {
            should_stop: Arc::clone(&self.should_stop.clone()),
        }
    }
}
