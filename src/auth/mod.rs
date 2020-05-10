use async_trait::async_trait;

#[async_trait]
trait Authentication<User> {
    async fn login(&self, u: User) -> Result<String>;
}

trait Authorization {
}

#[async_trait]
trait Authentication {
    async fn start_login_session(&self, hostname: String) -> Result<LoginSession>;
    async fn take_login_session(&self, identifier: String) -> Result<Option<LoginSession>>;

    async fn get_user(&self, token: String) -> Result<Option<User>>;
    async fn can_read(&self, user: &User, package: String) -> Result<bool>;
    async fn can_write(&self, user: &User, package: String) -> Result<bool>;
    async fn create_package(&self, user: &User, package: String) -> Result<Packument>;
}


