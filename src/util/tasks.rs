use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, mpsc};
use std::thread::{self, JoinHandle};

#[cfg(feature = "task-counter")]
use std::sync::atomic::AtomicU16;

use crate::excuses::INIT_ERR;
use crate::{cold_expression, library};

pub type BoxedTask = Box<dyn FnOnce() + Send + 'static>;

/// A simple thread pool implementation inspired by the Rust book:
/// <https://doc.rust-lang.org/book/ch21-02-multithreaded.html#building-threadpool-using-compiler-driven-development>
///
/// Note: The `Runner` can be shut down cleanly using the `shutdown`
/// method, otherwise the threads will be stopped immediately on drop
pub struct Runner {
    request: mpsc::Sender<BoxedTask>,
    threads: Vec<JoinHandle<()>>,
    waiting: Arc<AtomicBool>,
}

impl Runner {
    /// Creates a new instance of `Runner` with the specified
    /// number of worker threads (must be at least 1)
    ///
    /// # Panics
    /// The function panics if any threads fail to spawn
    #[inline]
    #[must_use]
    pub fn new(count: usize) -> Self {
        debug_assert!(count > 0, "Cannot create a thread pool with no threads");

        #[cfg(feature = "task-counter")]
        let busy_count = Arc::new(AtomicU16::new(0));

        let (tx, rx) = mpsc::channel::<BoxedTask>();
        let rx = Arc::new(Mutex::new(rx));
        let threads = (0..count).map(|i| {
            #[cfg(feature = "task-counter")]
            let busy_count = Arc::clone(&busy_count);

            let rx = Arc::clone(&rx);
            thread::Builder::new()
                .name(format!("worker_{i}"))
                .spawn(move || {
                    loop {
                        let Ok(task) = rx.lock().unwrap().recv() else {
                            break println!("Worker #{i} has quit"); // Breaking news!!
                        };

                        #[cfg(feature = "task-counter")]
                        {
                            busy_count.fetch_add(1, Ordering::Release);
                            println!(
                                "Background task started  (busy: {}/{count})",
                                busy_count.load(Ordering::Acquire)
                            );
                        }

                        task();

                        #[cfg(feature = "task-counter")]
                        {
                            busy_count.fetch_sub(1, Ordering::Release);
                            println!(
                                "Background task finished (busy: {}/{count})",
                                busy_count.load(Ordering::Acquire)
                            );
                        }
                    }
                })
                .expect(INIT_ERR)
        });
        Self {
            request: tx,
            threads: threads.collect(),
            waiting: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Runs a new task on the thread pool. If all workers
    /// are busy, the task will wait in a queue.
    #[inline]
    pub fn run<T>(&self, task: T)
    where
        T: FnOnce() + Into<Box<T>> + Send + 'static,
    {
        if let Err(e) = self.request.send(task.into()) {
            cold_expression! { eprintln!("Could not send task to the thread pool: {e}") };
        }
    }

    /// Any tasks requested after this function call will wait
    /// in a queue until all current ones have finished running
    ///
    /// Once all tasks have finished, `library::STATE` will be
    /// set to `STATE_READY`
    ///
    /// Calling this function when already waiting does nothing
    pub fn await_all_tasks(&self) {
        if self.waiting.swap(true, Ordering::Acquire) {
            return;
        }

        let (unblock_tx, unblock_rx) = mpsc::channel();
        let unblock_rx = Arc::new(Mutex::new(unblock_rx));
        let num_tasks = self.threads.len();

        #[cfg(feature = "verbose-logs")]
        println!("Awaiting background tasks");

        // Occupy all but one of the workers with a blocking operation
        for _ in 1..num_tasks {
            let unblock_rx = Arc::clone(&unblock_rx);
            #[allow(clippy::missing_panics_doc)]
            self.run(move || unblock_rx.lock().unwrap().recv().unwrap());
        }

        // When this task gets its turn in the queue, all tasks
        // started prior to this function have finished running
        let waiting = Arc::clone(&self.waiting);
        self.run(move || {
            // Notify the other workers to stop waiting
            for _ in 1..num_tasks {
                let _ = unblock_tx.send(());
            }

            waiting.store(false, Ordering::Release);
            library::STATE.store(library::STATE_READY, Ordering::Release);

            #[cfg(feature = "verbose-logs")]
            println!("Thread pool workers are ready");
        });
    }

    /// Consumes `self` and cleanly shuts down the worker threads
    /// by blocking until all tasks have finished running
    #[inline]
    pub fn shutdown(self) {
        drop(self.request);
        for thread in self.threads {
            let _ = thread.join();
        }
    }

    /// Returns the number of worker threads assigned to this thread pool
    #[inline]
    #[must_use]
    pub const fn num_workers(&self) -> usize {
        self.threads.len()
    }
}
