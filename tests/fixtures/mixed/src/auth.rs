pub trait Repository { fn save(&self); }
pub struct User;
impl Repository for User { fn save(&self) {} }
impl User { pub fn new() -> Self { Self } }
pub fn verify_password() -> bool { true }
pub fn login() { let user = User::new(); user.save(); verify_password(); }
