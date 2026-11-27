use core::mem;
use core::sync::atomic::{AtomicU8, Ordering};
use std::sync::Mutex;
use std::thread;
use std::time::Duration;

use crate::library::{Library, library_tx};

static SEQUENTIAL_TASKS: SequentialTasks = SequentialTasks::new();

#[inline]
#[must_use]
pub const fn sequential_tasks() -> &'static SequentialTasks {
    &SEQUENTIAL_TASKS
}

/// Runs tasks in sequence on the `Runner` thread pool, and allows
/// for cancellation inbetween tasks
pub struct SequentialTasks {
    tasks: Mutex<Vec<Box<dyn FnOnce() + Send + 'static>>>,
    state: AtomicU8,
}

const RUNNING: u8 = 0b01;
const CLOSED: u8 = 0b10;

impl SequentialTasks {
    #[inline]
    const fn new() -> Self {
        SequentialTasks {
            tasks: Mutex::new(Vec::new()),
            state: AtomicU8::new(0),
        }
    }

    /// Runs the task once it gets its turn in the sequential task queue
    ///
    /// Tasks are processed on the `Runner` thread pool (using `Library::run_task`), and
    /// the ttask will stop once the queue is empty (only one task will run at a time)
    pub fn run_in_queue<F: FnOnce() + Send + 'static>(&self, task: F) {
        if self.state.load(Ordering::Relaxed) & CLOSED != 0 {
            #[cfg(feature = "verbose-logs")]
            println!("Queue is closed - sequential task will not run");
            return;
        }
        let task_queue = &mut *self.tasks.lock().unwrap();
        task_queue.push(Box::new(task));
        if self.state.fetch_or(RUNNING, Ordering::Acquire) == 0 {
            let task_queue = mem::take(&mut *task_queue);
            Library::run_task(library_tx(), || sequential_tasks().run_all(task_queue));
        }
    }

    /// Runs all pending tasks, including tasks added while this function was running
    fn run_all(&self, mut queue: Vec<Box<dyn FnOnce() + Send + 'static>>) {
        #[cfg(feature = "verbose-logs")]
        println!("Running {} sequential tasks", queue.len());

        'run: loop {
            for task in queue {
                if self.state.load(Ordering::Relaxed) & CLOSED != 0 {
                    #[cfg(feature = "verbose-logs")]
                    println!("Sequential tasks have been cancelled");
                    return;
                }
                task();
            }

            // Check for and run new tasks `iter` times with a short delay inbetween
            // so more of them can be processed in sequence
            let mut iter = 3;
            loop {
                thread::sleep(Duration::from_millis(30));
                let mut new_tasks = self.tasks.lock().unwrap();
                if !new_tasks.is_empty() {
                    #[cfg(feature = "verbose-logs")]
                    println!("Running {} more sequential tasks", new_tasks.len());
                    queue = mem::take(&mut *new_tasks);
                    continue 'run;
                }
                iter -= 1;
                if iter == 0 {
                    self.state.fetch_and(!RUNNING, Ordering::Release);
                    #[cfg(feature = "verbose-logs")]
                    println!("All sequential tasks were run");
                    return;
                }
            }
        }
    }

    /// Stops running tasks, including those already waiting to run
    /// (tasks which have already started will not be interrupted)
    #[inline]
    pub fn stop_and_close_queue(&self) {
        self.state.store(CLOSED, Ordering::Release);
    }
}
