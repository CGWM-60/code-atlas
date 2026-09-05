pub struct User {
    pub id: u64,
    pub name: String,
}


pub enum Role {
    Admin,
    User,
}


pub trait Repository {

    fn save(&self);

}


impl User {

    pub fn new(
        id: u64,
        name: String,
    ) -> Self {

        Self {
            id,
            name,
        }
    }


    pub fn display_name(
        &self,
    ) -> &str {

        &self.name
    }
}


pub fn create_default_user() -> User {

    User::new(
        1,
        "Julien".to_string(),
    )
}