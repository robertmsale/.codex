#[cfg(all(target_os = "macos", not(target_arch = "wasm32")))]
mod platform {
    use std::collections::HashMap;
    use std::io::{Read, Write};
    use std::sync::{
        mpsc,
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    };
    use std::thread;

    use anyhow::{Result, anyhow};
    use portable_pty::{Child, CommandBuilder, MasterPty, PtySize, native_pty_system};
    use rinf::RustSignal;

    use crate::signals::TerminalEventSignal;

    pub struct TerminalRegistry {
        sessions: HashMap<String, TerminalSession>,
        cleanup_tx: mpsc::Sender<String>,
        cleanup_rx: mpsc::Receiver<String>,
        next_id: u64,
    }

    struct TerminalSession {
        host: String,
        username: String,
        master: Box<dyn MasterPty + Send>,
        writer: Arc<Mutex<Box<dyn Write + Send>>>,
        child: Box<dyn Child + Send + Sync>,
        closed: Arc<AtomicBool>,
    }

    impl TerminalRegistry {
        pub fn new() -> Self {
            let (cleanup_tx, cleanup_rx) = mpsc::channel();
            Self {
                sessions: HashMap::new(),
                cleanup_tx,
                cleanup_rx,
                next_id: 1,
            }
        }

        pub fn reap_finished(&mut self) {
            while let Ok(session_id) = self.cleanup_rx.try_recv() {
                self.reap_finished_session(&session_id);
            }
        }

        pub fn open(
            &mut self,
            request_id: String,
            host: String,
            username: String,
            cols: u32,
            rows: u32,
        ) -> Result<()> {
            let host = host.trim().to_string();
            if host.is_empty() {
                return Err(anyhow!("Terminal host is required"));
            }
            let username = username.trim().to_string();
            let session_id = format!("ssh-{}", self.next_id);
            self.next_id += 1;

            let pty = native_pty_system();
            let pair = pty.openpty(PtySize {
                rows: rows.max(8) as u16,
                cols: cols.max(20) as u16,
                pixel_width: 0,
                pixel_height: 0,
            })?;

            let mut command = CommandBuilder::new("/usr/bin/ssh");
            command.arg("-o");
            command.arg("PasswordAuthentication=no");
            command.arg("-o");
            command.arg("KbdInteractiveAuthentication=no");
            command.env("TERM", "xterm-256color");
            let destination = if username.is_empty() {
                host.clone()
            } else {
                format!("{username}@{host}")
            };
            command.arg(destination);

            let child = pair.slave.spawn_command(command)?;
            drop(pair.slave);

            let mut reader = pair.master.try_clone_reader()?;
            let writer = Arc::new(Mutex::new(pair.master.take_writer()?));
            let closed = Arc::new(AtomicBool::new(false));
            let read_closed = Arc::clone(&closed);
            let cleanup_tx = self.cleanup_tx.clone();
            let read_request_id = request_id.clone();
            let read_session_id = session_id.clone();
            let read_host = host.clone();
            let read_username = username.clone();
            thread::spawn(move || {
                let mut buffer = [0_u8; 8192];
                loop {
                    match reader.read(&mut buffer) {
                        Ok(0) => break,
                        Ok(count) => {
                            let data = String::from_utf8_lossy(&buffer[..count]).into_owned();
                            TerminalEventSignal {
                                request_id: read_request_id.clone(),
                                session_id: read_session_id.clone(),
                                kind: "output".to_string(),
                                data,
                                host: read_host.clone(),
                                username: read_username.clone(),
                            }
                            .send_signal_to_dart();
                        }
                        Err(error) => {
                            if !read_closed.load(Ordering::Relaxed) {
                                TerminalEventSignal {
                                    request_id: read_request_id.clone(),
                                    session_id: read_session_id.clone(),
                                    kind: "error".to_string(),
                                    data: error.to_string(),
                                    host: read_host.clone(),
                                    username: read_username.clone(),
                                }
                                .send_signal_to_dart();
                            }
                            break;
                        }
                    }
                }
                TerminalEventSignal {
                    request_id: read_request_id.clone(),
                    session_id: read_session_id.clone(),
                    kind: "closed".to_string(),
                    data: String::new(),
                    host: read_host,
                    username: read_username,
                }
                .send_signal_to_dart();
                let _ = cleanup_tx.send(read_session_id);
            });

            self.sessions.insert(
                session_id.clone(),
                TerminalSession {
                    host: host.clone(),
                    username: username.clone(),
                    master: pair.master,
                    writer,
                    child,
                    closed,
                },
            );

            TerminalEventSignal {
                request_id,
                session_id,
                kind: "opened".to_string(),
                data: String::new(),
                host,
                username,
            }
            .send_signal_to_dart();
            Ok(())
        }

        pub fn input(&mut self, session_id: &str, data: &str) -> Result<()> {
            let session = self
                .sessions
                .get_mut(session_id)
                .ok_or_else(|| anyhow!("Unknown terminal session"))?;
            let mut writer = session
                .writer
                .lock()
                .map_err(|_| anyhow!("Terminal writer is unavailable"))?;
            writer.write_all(data.as_bytes())?;
            writer.flush()?;
            Ok(())
        }

        pub fn resize(&mut self, session_id: &str, cols: u32, rows: u32) -> Result<()> {
            let session = self
                .sessions
                .get_mut(session_id)
                .ok_or_else(|| anyhow!("Unknown terminal session"))?;
            session.master.resize(PtySize {
                rows: rows.max(8) as u16,
                cols: cols.max(20) as u16,
                pixel_width: 0,
                pixel_height: 0,
            })?;
            Ok(())
        }

        pub fn close(&mut self, session_id: &str) {
            if let Some(mut session) = self.sessions.remove(session_id) {
                session.closed.store(true, Ordering::Relaxed);
                let _ = session.child.kill();
                let _ = session.child.wait();
                TerminalEventSignal {
                    request_id: String::new(),
                    session_id: session_id.to_string(),
                    kind: "closed".to_string(),
                    data: String::new(),
                    host: session.host,
                    username: session.username,
                }
                .send_signal_to_dart();
            }
        }

        fn reap_finished_session(&mut self, session_id: &str) {
            if let Some(mut session) = self.sessions.remove(session_id) {
                session.closed.store(true, Ordering::Relaxed);
                let _ = session.child.wait();
            }
        }

        pub fn close_all(&mut self) {
            let session_ids = self.sessions.keys().cloned().collect::<Vec<_>>();
            for session_id in session_ids {
                self.close(&session_id);
            }
        }
    }

    impl Drop for TerminalRegistry {
        fn drop(&mut self) {
            self.close_all();
        }
    }
}

#[cfg(not(all(target_os = "macos", not(target_arch = "wasm32"))))]
mod platform {
    use anyhow::{Result, anyhow};

    pub struct TerminalRegistry;

    impl TerminalRegistry {
        pub fn new() -> Self {
            Self
        }

        pub fn reap_finished(&mut self) {}

        pub fn open(
            &mut self,
            _request_id: String,
            _host: String,
            _username: String,
            _cols: u32,
            _rows: u32,
        ) -> Result<()> {
            Err(anyhow!("Integrated terminal is only available on macOS desktop"))
        }

        pub fn input(&mut self, _session_id: &str, _data: &str) -> Result<()> {
            Ok(())
        }

        pub fn resize(&mut self, _session_id: &str, _cols: u32, _rows: u32) -> Result<()> {
            Ok(())
        }

        pub fn close(&mut self, _session_id: &str) {}

        pub fn close_all(&mut self) {}
    }
}

pub use platform::TerminalRegistry;
