use crate::BaseCase;
pub struct BehaveSbContext {
    pub sb: Option<BaseCase>,
}
impl BehaveSbContext {
    pub fn new() -> Self {
        Self { sb: None }
    }
}
