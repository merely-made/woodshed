use super::{ChordRender, SampleBuffer, Wake};
use std::sync::{
    Arc, Condvar, Mutex,
    atomic::{AtomicU64, Ordering},
};

struct Request {
    params: ChordRender,
    rate: u32,
    revision: u64,
}
pub(super) struct Completion {
    pub buffer: SampleBuffer,
    pub revision: u64,
}
#[derive(Default)]
struct Queue {
    pending: Option<Request>,
    completed: Option<Completion>,
    running: bool,
    closed: bool,
    wake: Option<Wake>,
}
struct Mailbox {
    queue: Mutex<Queue>,
    changed: Condvar,
    revision: AtomicU64,
}
pub(super) struct PreviewWorker {
    mailbox: Arc<Mailbox>,
    worker: Option<std::thread::JoinHandle<()>>,
}
impl PreviewWorker {
    pub fn new() -> Self {
        let mailbox = Arc::new(Mailbox {
            queue: Mutex::new(Queue::default()),
            changed: Condvar::new(),
            revision: AtomicU64::new(0),
        });
        let thread_mailbox = mailbox.clone();
        let worker = std::thread::Builder::new()
            .name("woodshed-preview".into())
            .spawn(move || {
                loop {
                    let request = {
                        let mut q = thread_mailbox.queue.lock().unwrap();
                        while q.pending.is_none() && !q.closed {
                            q = thread_mailbox.changed.wait(q).unwrap();
                        }
                        if q.closed {
                            break;
                        }
                        q.running = true;
                        q.pending.take().unwrap()
                    };
                    let cancelled =
                        || thread_mailbox.revision.load(Ordering::Acquire) != request.revision;
                    let buffer = crate::chord_audio::render_chord_cancellable(
                        &request.params,
                        request.rate,
                        &cancelled,
                    );
                    let wake = {
                        let mut q = thread_mailbox.queue.lock().unwrap();
                        q.running = false;
                        if !q.closed && !cancelled() {
                            if let Some(buffer) = buffer {
                                q.completed = Some(Completion {
                                    buffer,
                                    revision: request.revision,
                                });
                            }
                        }
                        q.wake.clone()
                    };
                    if let Some(wake) = wake {
                        wake();
                    }
                }
            })
            .expect("preview worker creation");
        Self {
            mailbox,
            worker: Some(worker),
        }
    }
    pub fn set_wake(&self, wake: Wake) {
        self.mailbox.queue.lock().unwrap().wake = Some(wake);
    }
    pub fn busy(&self) -> bool {
        let q = self.mailbox.queue.lock().unwrap();
        q.running || q.pending.is_some() || q.completed.is_some()
    }
    pub fn request(&self, params: ChordRender, rate: u32) -> u64 {
        let revision = self.cancel();
        self.mailbox.queue.lock().unwrap().pending = Some(Request {
            params,
            rate,
            revision,
        });
        self.mailbox.changed.notify_one();
        revision
    }
    pub fn cancel(&self) -> u64 {
        let revision = self
            .mailbox
            .revision
            .fetch_add(1, Ordering::AcqRel)
            .wrapping_add(1);
        let mut q = self.mailbox.queue.lock().unwrap();
        q.pending = None;
        q.completed = None;
        revision
    }
    pub fn take_completion(&self) -> Option<Completion> {
        self.mailbox.queue.lock().unwrap().completed.take()
    }
}
impl Drop for PreviewWorker {
    fn drop(&mut self) {
        self.cancel();
        {
            let mut q = self.mailbox.queue.lock().unwrap();
            q.closed = true;
        }
        self.mailbox.changed.notify_one();
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}
