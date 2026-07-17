#[derive(Debug)]
pub enum SbException {
    TimeoutException(String),
    NoSuchElementException(String),
    NotInteractableException(String),
}
