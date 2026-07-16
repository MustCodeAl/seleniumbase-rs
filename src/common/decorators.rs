// Rust doesn't have Python decorators, but we can implement macros or wrapper functions
pub fn retry_on_exception<F, T, E>(mut f: F, retries: u32) -> Result<T, E>
where
    F: FnMut() -> Result<T, E>,
{
    let mut attempts = 0;
    loop {
        match f() {
            Ok(val) => return Ok(val),
            Err(e) => {
                attempts += 1;
                if attempts >= retries {
                    return Err(e);
                }
            }
        }
    }
}
