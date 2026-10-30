use std::{
    fmt::Display,
    time::{Duration, Instant},
};

/// Minimum, maximum, and average of a set of [`Benchmark`] measurements.
pub struct TotalBenchmark {
    /// Minimum observed measurement for each timing component.
    pub min: Benchmark,
    /// Maximum observed measurement for each timing component.
    pub max: Benchmark,
    /// Arithmetic mean measurement for each timing component.
    pub avg: Benchmark,
}

impl Display for TotalBenchmark {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!(
            "Min:\n{}\n\nMax:\n{}\n\nAvg:\n{}",
            self.min, self.max, self.avg
        ))
    }
}

impl From<Vec<Benchmark>> for TotalBenchmark {
    fn from(results: Vec<Benchmark>) -> Self {
        let len = results.len() as u32;
        if len == 0 {
            return Self {
                min: Default::default(),
                max: Default::default(),
                avg: Default::default(),
            };
        }
        let (results_total, (results_engine, results_driver)): (Vec<_>, (Vec<_>, Vec<_>)) = results
            .into_iter()
            .map(
                |Benchmark {
                     total_time,
                     engine_time,
                     driver_time,
                 }| (total_time, (engine_time, driver_time)),
            )
            .unzip();

        let min = Benchmark {
            total_time: results_total.iter().min().copied().unwrap(),
            engine_time: results_engine.iter().min().copied().unwrap(),
            driver_time: results_driver.iter().min().copied().unwrap(),
        };
        let max = Benchmark {
            total_time: results_total.iter().max().copied().unwrap(),
            engine_time: results_engine.iter().max().copied().unwrap(),
            driver_time: results_driver.iter().max().copied().unwrap(),
        };
        let avg = Benchmark {
            total_time: results_total.iter().sum::<Duration>() / len,
            engine_time: results_engine.iter().sum::<Duration>() / len,
            driver_time: results_driver.iter().sum::<Duration>() / len,
        };

        Self { min, max, avg }
    }
}

/// Timing breakdown for one measured ORM operation or workload.
#[derive(Default)]
pub struct Benchmark {
    /// Total elapsed time measured around the ORM operation.
    pub total_time: Duration,
    /// Time spent in ORM orchestration and transformations.
    pub engine_time: Duration,
    /// Time measured while awaiting driver operations.
    pub driver_time: Duration,
}

impl Display for Benchmark {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!("Total time elapsed: {:?}\nTime elapsed on engine operations: {:?}\nTime elapsed on driver operations: {:?}", self.total_time, self.engine_time, self.driver_time))
    }
}
pub(crate) struct BenchmarkRecorder {
    pub total_time: Duration,
    pub driver_time: Duration,
}

impl BenchmarkRecorder {
    pub(crate) fn new() -> Self {
        Self {
            total_time: Duration::ZERO,
            driver_time: Duration::ZERO,
        }
    }

    pub(crate) fn tool(&self) -> BenchmarkTool {
        BenchmarkTool::new()
    }

    pub(crate) fn add(&mut self, total_time: Duration, driver_time: Duration) {
        self.total_time += total_time;
        self.driver_time += driver_time;
    }

    pub(crate) fn finish(self) -> Benchmark {
        let Self {
            total_time,
            driver_time,
        } = self;

        let engine_time = total_time - driver_time;

        Benchmark {
            total_time,
            engine_time,
            driver_time,
        }
    }
}

pub(crate) struct BenchmarkTool {
    pub total_instant: Instant,
    pub driver_instant: Option<Instant>,
    pub driver_time: Duration,
}

impl BenchmarkTool {
    pub(crate) fn new() -> Self {
        Self {
            driver_instant: None,
            driver_time: Duration::ZERO,
            total_instant: Instant::now(),
        }
    }

    pub(crate) fn driver_start(&mut self) {
        self.driver_instant = Some(Instant::now());
    }

    pub(crate) fn driver_end(&mut self) {
        if let Some(instant) = self.driver_instant.take() {
            self.driver_time += instant.elapsed();
        }
    }

    pub(crate) fn finish(self, recorder: &mut BenchmarkRecorder) {
        let total_time = self.total_instant.elapsed();
        recorder.add(total_time, self.driver_time)
    }
}
