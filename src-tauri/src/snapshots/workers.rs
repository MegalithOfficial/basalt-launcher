use std::sync::{
    atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering},
    Mutex,
};

use crate::{
    error::{Error, Result},
    tasks::TaskHandle,
};

pub(super) struct Progress {
    completed: AtomicU64,
    processed_bytes: AtomicU64,
    total_files: u64,
    total_bytes: u64,
    report_lock: Mutex<()>,
}

impl Progress {
    pub(super) fn new(total_files: u64, total_bytes: u64, task: Option<&TaskHandle>) -> Self {
        if let Some(task) = task {
            task.set_total(total_files, total_bytes);
        }
        Self {
            completed: AtomicU64::new(0),
            processed_bytes: AtomicU64::new(0),
            total_files,
            total_bytes,
            report_lock: Mutex::new(()),
        }
    }

    pub(super) fn bytes(&self, count: usize, task: Option<&TaskHandle>) {
        self.processed_bytes
            .fetch_add(count as u64, Ordering::Relaxed);
        self.report(task);
    }

    pub(super) fn bytes_u64(&self, count: u64, task: Option<&TaskHandle>) {
        self.processed_bytes.fetch_add(count, Ordering::Relaxed);
        self.report(task);
    }

    pub(super) fn file(&self, task: Option<&TaskHandle>) {
        self.completed.fetch_add(1, Ordering::Relaxed);
        self.report(task);
    }

    fn report(&self, task: Option<&TaskHandle>) {
        if let Some(task) = task {
            let _guard = self.report_lock.lock().unwrap();
            task.progress(
                self.completed.load(Ordering::Relaxed),
                self.total_files,
                self.processed_bytes.load(Ordering::Relaxed),
                self.total_bytes,
            );
        }
    }
}

pub(super) fn worker_count(items: usize) -> usize {
    if items == 0 {
        return 0;
    }
    std::thread::available_parallelism()
        .map(usize::from)
        .unwrap_or(1)
        .min(16)
        .min(items)
}

pub(super) fn parallel_map<T, R, F>(items: &[T], work: F) -> Result<Vec<R>>
where
    T: Sync,
    R: Send,
    F: Fn(&T) -> Result<R> + Sync,
{
    let workers = worker_count(items.len());
    if workers == 0 {
        return Ok(Vec::new());
    }

    let next = AtomicUsize::new(0);
    let stopped = AtomicBool::new(false);
    let results = Mutex::new(
        std::iter::repeat_with(|| None)
            .take(items.len())
            .collect::<Vec<Option<R>>>(),
    );
    let first_error = Mutex::new(None);

    std::thread::scope(|scope| -> Result<()> {
        let mut handles = Vec::with_capacity(workers);
        for _ in 0..workers {
            handles.push(scope.spawn(|| loop {
                if stopped.load(Ordering::Acquire) {
                    break;
                }
                let index = next.fetch_add(1, Ordering::Relaxed);
                let Some(item) = items.get(index) else {
                    break;
                };
                match work(item) {
                    Ok(value) => results.lock().unwrap()[index] = Some(value),
                    Err(error) => {
                        stopped.store(true, Ordering::Release);
                        let mut first = first_error.lock().unwrap();
                        if first.is_none() {
                            *first = Some(error);
                        }
                        break;
                    }
                }
            }));
        }
        for handle in handles {
            if handle.join().is_err() {
                stopped.store(true, Ordering::Release);
                return Err(Error::other("snapshot worker panicked"));
            }
        }
        Ok(())
    })?;

    if let Some(error) = first_error.into_inner().unwrap() {
        return Err(error);
    }
    results
        .into_inner()
        .unwrap()
        .into_iter()
        .map(|result| result.ok_or_else(|| Error::other("snapshot worker stopped early")))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Barrier;

    #[test]
    fn parallel_map_preserves_input_order() {
        let values = (0..128).collect::<Vec<_>>();
        let mapped = parallel_map(&values, |value| Ok(value * 2)).unwrap();
        assert_eq!(
            mapped,
            values.iter().map(|value| value * 2).collect::<Vec<_>>()
        );
    }

    #[test]
    fn worker_count_is_bounded() {
        assert_eq!(worker_count(0), 0);
        assert!((1..=16).contains(&worker_count(100)));
        assert!(worker_count(2) <= 2);
    }

    #[test]
    fn work_runs_concurrently_when_multiple_cpus_are_available() {
        let workers = worker_count(64);
        if workers < 2 {
            return;
        }
        let values = (0..workers).collect::<Vec<_>>();
        let barrier = Barrier::new(workers);
        let mapped = parallel_map(&values, |value| {
            barrier.wait();
            Ok(*value)
        })
        .unwrap();
        assert_eq!(mapped, values);
    }

    #[test]
    fn first_worker_error_stops_the_batch() {
        let result = parallel_map(&(0..64).collect::<Vec<_>>(), |value| {
            if *value == 0 {
                Err(Error::other("failed worker"))
            } else {
                Ok(*value)
            }
        });
        assert!(result.is_err());
    }
}
