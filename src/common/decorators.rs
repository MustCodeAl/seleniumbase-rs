use std::cell::RefCell;
use std::future::Future;
use std::rc::Rc;
use std::time::{Duration, Instant};

pub fn retry_on_exception<F, T, E>(max_attempts: u32, mut f: F) -> Result<T, E>
where
    F: FnMut() -> Result<T, E>,
{
    assert!(max_attempts > 0, "max_attempts must be greater than zero");
    for attempt in 1..=max_attempts {
        match f() {
            Ok(value) => return Ok(value),
            Err(err) if attempt == max_attempts => return Err(err),
            Err(_) => std::thread::sleep(Duration::from_millis(100)),
        }
    }
    unreachable!()
}

pub async fn retry_on_exception_async<F, Fut, T, E>(max_attempts: u32, mut f: F) -> Result<T, E>
where
    F: FnMut() -> Fut,
    Fut: Future<Output = Result<T, E>>,
{
    assert!(max_attempts > 0, "max_attempts must be greater than zero");
    for attempt in 1..=max_attempts {
        match f().await {
            Ok(value) => return Ok(value),
            Err(err) if attempt == max_attempts => return Err(err),
            Err(_) => tokio::time::sleep(Duration::from_millis(100)).await,
        }
    }
    unreachable!()
}

pub fn rate_limited<F, T>(max_calls: u32, window_secs: u64, f: F) -> impl FnMut() -> T
where
    F: FnMut() -> T,
{
    assert!(max_calls > 0, "max_calls must be greater than zero");
    let window = Duration::from_secs(window_secs);
    let calls: Rc<RefCell<Vec<Instant>>> = Rc::new(RefCell::new(Vec::new()));
    let f = Rc::new(RefCell::new(f));
    move || {
        let now = Instant::now();
        let mut times = calls.borrow_mut();
        times.retain(|t| now.duration_since(*t) < window);
        if times.len() >= max_calls as usize {
            let elapsed = now.duration_since(times[0]);
            let sleep = if elapsed < window {
                window - elapsed
            } else {
                Duration::ZERO
            };
            std::thread::sleep(sleep);
            let now = Instant::now();
            times.retain(|t| now.duration_since(*t) < window);
            times.push(now);
        } else {
            times.push(now);
        }
        f.borrow_mut()()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retry_on_exception_succeeds_after_failures() {
        let mut attempts = 0;
        let result = retry_on_exception(3, || {
            attempts += 1;
            if attempts < 3 {
                Err("fail")
            } else {
                Ok("ok")
            }
        });
        assert_eq!(result, Ok("ok"));
        assert_eq!(attempts, 3);
    }

    #[test]
    fn retry_on_exception_returns_last_error() {
        let result: Result<&str, &str> = retry_on_exception(2, || Err("always fails"));
        assert_eq!(result, Err("always fails"));
    }

    #[tokio::test]
    async fn retry_on_exception_async_succeeds_after_failures() {
        let attempts = std::sync::Arc::new(std::sync::Mutex::new(0));
        let result = retry_on_exception_async(3, {
            let attempts = attempts.clone();
            move || {
                let a = {
                    let mut guard = attempts.lock().unwrap();
                    *guard += 1;
                    *guard
                };
                async move {
                    if a < 3 {
                        Err::<String, String>("fail".to_string())
                    } else {
                        Ok("ok".to_string())
                    }
                }
            }
        })
        .await;
        assert_eq!(result, Ok("ok".to_string()));
    }

    #[test]
    fn rate_limited_enforces_window() {
        let start = Instant::now();
        let mut counter = 0;
        let mut limited = rate_limited(2, 1, || {
            counter += 1;
            counter
        });
        limited();
        limited();
        let third = limited();
        let elapsed = start.elapsed();
        assert_eq!(third, 3);
        assert!(
            elapsed >= Duration::from_secs(1),
            "expected delay, got {:?}",
            elapsed
        );
    }

    #[test]
    fn rate_limited_allows_calls_within_limit() {
        let mut counter = 0;
        let mut limited = rate_limited(3, 1, || {
            counter += 1;
            counter
        });
        assert_eq!(limited(), 1);
        assert_eq!(limited(), 2);
        assert_eq!(limited(), 3);
    }
}
