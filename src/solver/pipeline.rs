use std::thread;

use crossbeam_channel::{bounded, unbounded, Receiver, Sender};

use crate::{
    settings::{READ_CALC_RATIO, WRITE_BUFFER_SIZE},
    util::{get_num_calc_workers, get_num_read_workers},
};

/// How full a channel was each time something was received from it.
#[derive(Debug, Clone, Copy)]
pub struct QueueStats {
    total_len: usize,
    received: usize,
    capacity: usize,
}

impl QueueStats {
    fn merge(self, other: Self) -> Self {
        Self {
            total_len: self.total_len + other.total_len,
            received: self.received + other.received,
            capacity: self.capacity,
        }
    }

    pub fn avg_len(&self) -> f64 {
        self.total_len as f64 / self.received.max(1) as f64
    }

    pub fn avg_fullness(&self) -> f64 {
        self.avg_len() / self.capacity as f64
    }
}

#[derive(Debug, Clone, Copy)]
pub struct PipelineStats {
    /// Read results waiting for a calc worker
    pub calc_queue: QueueStats,
    /// Calc results waiting for the writer
    pub write_queue: QueueStats,
}

/// Calls `f` on everything received, until every sender is gone.
fn receive_all<T>(receiver: Receiver<T>, mut f: impl FnMut(T)) -> QueueStats {
    let mut stats = QueueStats {
        total_len: 0,
        received: 0,
        capacity: receiver.capacity().unwrap(),
    };
    while let Ok(item) = receiver.recv() {
        stats.received += 1;
        stats.total_len += receiver.len() + 1;
        f(item);
    }
    stats
}

/// Runs every task through the read workers, then the calc workers, then one writer. Returns once everything is
/// written. Panics if any of the workers did.
///
/// A stage stops when every sender to it is gone (no more input), or every receiver after it is (the next stage
/// panicked). So only the workers can hold them: a sender or receiver kept anywhere else can make this wait forever.
pub fn run_pipeline<T: Send, R: Send, W: Send>(
    tasks: impl Iterator<Item = T>,
    read: impl Fn(T) -> R + Sync,
    calc: impl Fn(R, &Sender<W>) + Sync,
    write: impl FnMut(W) + Send,
) -> PipelineStats {
    let cpu_count = num_cpus::get();
    let num_read_workers = get_num_read_workers(cpu_count, READ_CALC_RATIO);
    let num_calc_workers = get_num_calc_workers(cpu_count, READ_CALC_RATIO);

    // Each channel is named after the stage that receives from it. Reading is unbounded so this thread doesn't have
    // to wake up for every task.
    let (read_sender, read_receiver) = unbounded();
    let (calc_sender, calc_receiver) = bounded(num_read_workers);
    let (write_sender, write_receiver) = bounded(WRITE_BUFFER_SIZE);

    thread::scope(|s| {
        let (read, calc) = (&read, &calc);
        for _ in 0..num_read_workers {
            let read_receiver = read_receiver.clone();
            let calc_sender = calc_sender.clone();
            s.spawn(move || {
                for task in read_receiver {
                    // Only fails if every calc worker panicked
                    if calc_sender.send(read(task)).is_err() {
                        break;
                    }
                }
            });
        }
        let mut calc_workers = Vec::new();
        for _ in 0..num_calc_workers {
            let calc_receiver = calc_receiver.clone();
            let write_sender = write_sender.clone();
            calc_workers.push(
                s.spawn(move || receive_all(calc_receiver, |result| calc(result, &write_sender))),
            );
        }
        let writer = s.spawn(move || receive_all(write_receiver, write));
        // The workers have their own clones
        drop((read_receiver, calc_sender, calc_receiver, write_sender));

        for task in tasks {
            // Only fails if every read worker panicked
            if read_sender.send(task).is_err() {
                break;
            }
        }
        drop(read_sender);

        PipelineStats {
            calc_queue: calc_workers
                .into_iter()
                .map(|worker| worker.join().expect("Calc worker panicked"))
                .reduce(QueueStats::merge)
                .unwrap(),
            write_queue: writer.join().expect("Writer panicked"),
        }
    })
}

// cargo test --release --lib pipeline
#[cfg(test)]
mod tests {
    use super::*;

    const NUM_TASKS: u32 = 10_000;

    fn send(result: u32, sender: &Sender<u32>) {
        sender.send(result).unwrap();
    }

    #[test]
    fn every_task_is_written_once() {
        let mut written = Vec::new();
        let stats = run_pipeline(
            0..NUM_TASKS,
            |task| task * 2,
            |result, sender| send(result + 1, sender),
            |item| written.push(item),
        );
        written.sort();
        assert_eq!(
            written,
            (0..NUM_TASKS).map(|i| i * 2 + 1).collect::<Vec<_>>()
        );
        assert_eq!(stats.calc_queue.received, NUM_TASKS as usize);
        assert_eq!(stats.write_queue.received, NUM_TASKS as usize);
    }

    #[test]
    fn no_tasks() {
        let stats = run_pipeline(0..0, |task: u32| task, send, |_| {});
        assert_eq!(stats.calc_queue.avg_len(), 0.0);
        assert_eq!(stats.write_queue.avg_fullness(), 0.0);
    }

    // A sender or receiver that isn't dropped can make these hang instead of fail

    #[test]
    #[should_panic]
    fn read_panic_on_every_task() {
        run_pipeline(0..NUM_TASKS, |_| panic!("read"), send, |_| {});
    }

    #[test]
    #[should_panic]
    fn read_panic_on_one_task() {
        let read = |task| if task == 5 { panic!("read") } else { task };
        run_pipeline(0..NUM_TASKS, read, send, |_| {});
    }

    // Fewer tasks than read workers, so most workers never panic
    #[test]
    #[should_panic]
    fn read_panic_on_the_only_task() {
        run_pipeline(0..1, |_| panic!("read"), send, |_| {});
    }

    #[test]
    #[should_panic]
    fn calc_panic() {
        let calc = |_, _: &Sender<u32>| panic!("calc");
        run_pipeline(0..NUM_TASKS, |task| task, calc, |_| {});
    }

    #[test]
    #[should_panic]
    fn write_panic() {
        run_pipeline(0..NUM_TASKS, |task| task, send, |_| panic!("write"));
    }
}
