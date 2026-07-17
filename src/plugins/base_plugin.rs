pub trait BasePlugin {
    fn setup(&self);
    fn teardown(&self);
}
