pub struct BehaveHelper;
impl BehaveHelper {
    pub fn before_all() {
        println!("behave before_all");
    }
    pub fn after_all() {
        println!("behave after_all");
    }
}
