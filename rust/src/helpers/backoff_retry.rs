use std::fmt::Display;
use std::time::Duration;

use backoff::{retry_notify, Error, ExponentialBackoff};

pub fn backoff_retry<F, T, E>(fn_to_try: F) -> Result<T, Error<E>>
where
    F: FnMut() -> Result<T, Error<E>>,
    E: Display,
{
    let notify = |err, dur: Duration| {
        log::error!(
            "Temporary error after {:.1}s: {}",
            dur.as_secs_f32(),
            err
        );
    };

    retry_notify(ExponentialBackoff::default(), fn_to_try, notify)
}
