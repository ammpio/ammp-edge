use std::fmt::Display;
use std::time::Duration;

use backoff;

pub fn backoff_retry<F, T, E>(
    fn_to_try: F,
    timeout: Option<Duration>,
) -> Result<T, backoff::Error<E>>
where
    F: FnMut() -> Result<T, backoff::Error<E>>,
    E: Display,
{
    let fn_name = std::any::type_name::<F>();

    let notify = |err, dur: Duration| {
        log::error!(
            "when running '{}' temporary error after {:.1}s: {}",
            fn_name,
            dur.as_secs_f32(),
            err
        );
    };

    // Set to retry forever, rather than give up after 15 minutes.
    // See https://github.com/ihrwein/backoff/issues/39
    let backoff = backoff::ExponentialBackoffBuilder::new()
        .with_max_elapsed_time(timeout)
        .build();
    backoff::retry_notify(backoff, fn_to_try, notify)
}
