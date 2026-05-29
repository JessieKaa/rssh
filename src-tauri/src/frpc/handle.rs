use std::collections::VecDeque;
use std::io::{BufRead, BufReader};
use std::process::Child;
use std::sync::{Arc, Mutex};

const LOG_RING_CAPACITY: usize = 200;

pub struct FrpcHandle {
    child: Arc<Mutex<Option<Child>>>,
    log_buffer: Arc<Mutex<VecDeque<String>>>,
    config_id: String,
}

impl FrpcHandle {
    pub fn new(child: Child, config_id: String) -> Self {
        let child = Arc::new(Mutex::new(Some(child)));
        let log_buffer: Arc<Mutex<VecDeque<String>>> = Arc::new(Mutex::new(VecDeque::new()));

        // Spawn reader threads for stdout and stderr
        let stdout = {
            let mut guard = child.lock().unwrap();
            guard.as_mut().and_then(|c| c.stdout.take())
        };
        let stderr = {
            let mut guard = child.lock().unwrap();
            guard.as_mut().and_then(|c| c.stderr.take())
        };

        if let Some(stdout) = stdout {
            let buf = log_buffer.clone();
            std::thread::spawn(move || {
                let reader = BufReader::new(stdout);
                for line in reader.lines() {
                    match line {
                        Ok(l) => {
                            let mut guard = buf.lock().unwrap();
                            if guard.len() >= LOG_RING_CAPACITY {
                                guard.pop_front();
                            }
                            guard.push_back(l);
                        }
                        Err(_) => break,
                    }
                }
            });
        }

        if let Some(stderr) = stderr {
            let buf = log_buffer.clone();
            std::thread::spawn(move || {
                let reader = BufReader::new(stderr);
                for line in reader.lines() {
                    match line {
                        Ok(l) => {
                            let mut guard = buf.lock().unwrap();
                            if guard.len() >= LOG_RING_CAPACITY {
                                guard.pop_front();
                            }
                            guard.push_back(l);
                        }
                        Err(_) => break,
                    }
                }
            });
        }

        Self {
            child,
            log_buffer,
            config_id,
        }
    }

    pub fn config_id(&self) -> &str {
        &self.config_id
    }

    pub fn stop(&self) {
        if let Ok(mut guard) = self.child.lock() {
            if let Some(mut child) = guard.take() {
                // Try SIGTERM first on Unix, then SIGKILL
                #[cfg(unix)]
                {
                    let _ = child.kill();
                    // Wait up to 2 seconds for graceful shutdown
                    let start = std::time::Instant::now();
                    loop {
                        match child.try_wait() {
                            Ok(Some(_)) => break,
                            Ok(None) => {
                                if start.elapsed() > std::time::Duration::from_secs(2) {
                                    let _ = child.kill();
                                    let _ = child.wait();
                                    break;
                                }
                                std::thread::sleep(std::time::Duration::from_millis(50));
                            }
                            Err(_) => break,
                        }
                    }
                }
                #[cfg(not(unix))]
                {
                    let _ = child.kill();
                    let _ = child.wait();
                }
            }
        }
    }

    pub fn is_running(&self) -> bool {
        if let Ok(mut guard) = self.child.lock() {
            if let Some(ref mut child) = *guard {
                return child.try_wait().ok().flatten().is_none();
            }
        }
        false
    }

    pub fn pid(&self) -> Option<u32> {
        if let Ok(guard) = self.child.lock() {
            if let Some(ref child) = *guard {
                return Some(child.id());
            }
        }
        None
    }

    pub fn logs(&self, limit: usize) -> Vec<String> {
        if let Ok(guard) = self.log_buffer.lock() {
            let skip = guard.len().saturating_sub(limit);
            guard.iter().skip(skip).cloned().collect()
        } else {
            Vec::new()
        }
    }
}
