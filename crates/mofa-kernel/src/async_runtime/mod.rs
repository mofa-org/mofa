use std::future::Future;
use std::time::Duration;

pub trait AsyncRuntime: Send + Sync {
    fn spawn(&self, task: impl Future<Output = ()> + Send + 'static);

    fn spawn_local(&self, task: impl Future<Output = ()> + 'static);

    fn sleep(&self, duration: Duration) -> SleepFuture;

    fn timeout<T>(&self, duration: Duration, future: impl Future<Output = T>) -> TimeoutFuture<T>;

    fn now(&self) -> std::time::Instant;
}

pub type SleepFuture = Pin<Box<dyn Future<Output = ()> + Send>>;
pub type TimeoutFuture<T> = Pin<Box<dyn Future<Output = Result<T, TimeoutError>> + Send>>;

#[derive(Debug)]
pub struct TimeoutError;

impl std::fmt::Display for TimeoutError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "operation timed out")
    }
}

impl std::error::Error for TimeoutError {}

pub mod tokio_runtime {
    use super::*;

    #[derive(Clone)]
    pub struct TokioRuntime;

    impl TokioRuntime {
        pub fn new() -> Self {
            Self
        }
    }

    impl Default for TokioRuntime {
        fn default() -> Self {
            Self::new()
        }
    }

    impl AsyncRuntime for TokioRuntime {
        fn spawn(&self, task: impl Future<Output = ()> + Send + 'static) {
            tokio::spawn(task);
        }

        fn spawn_local(&self, task: impl Future<Output = ()> + 'static) {
            tokio::task::spawn_local(task);
        }

        fn sleep(&self, duration: Duration) -> SleepFuture {
            Box::pin(tokio::time::sleep(duration))
        }

        fn timeout<T>(&self, duration: Duration, future: impl Future<Output = T>) -> TimeoutFuture<T> {
            Box::pin(tokio::time::timeout(duration, future))
        }

        fn now(&self) -> std::time::Instant {
            std::time::Instant::now()
        }
    }
}

pub mod runtime_builder {
    use super::*;

    pub enum RuntimeType {
        Tokio,
        #[cfg(feature = \"custom-runtime\")]
        Custom(Box<dyn AsyncRuntime>),
    }

    pub struct RuntimeBuilder {
        runtime_type: RuntimeType,
    }

    impl RuntimeBuilder {
        pub fn new() -> Self {
            Self {
                runtime_type: RuntimeType::Tokio,
            }
        }

        pub fn with_tokio(mut self) -> Self {
            self.runtime_type = RuntimeType::Tokio;
            self
        }

        #[cfg(feature = \"custom-runtime\")]
        pub fn with_custom(mut self, runtime: impl AsyncRuntime + 'static) -> Self {
            self.runtime_type = RuntimeType::Custom(Box::new(runtime));
            self
        }

        pub fn build(self) -> Box<dyn AsyncRuntime> {
            match self.runtime_type {
                RuntimeType::Tokio => Box::new(tokio_runtime::TokioRuntime::new()),
                #[cfg(feature = \"custom-runtime\")]
                RuntimeType::Custom(runtime) => runtime,
            }
        }
    }

    impl Default for RuntimeBuilder {
        fn default() -> Self {
            Self::new()
        }
    }
}
