use serde::Serialize;

use crate::models::User;

pub(crate) mod in_memory;

#[async_trait::async_trait]
pub trait UserStorage: Send + Sync {
    async fn register_user<U: Into<User> + Serialize + Send + Sync>(
        &self,
        user: U,
    ) -> anyhow::Result<User>;
    async fn get_user(&self, username: &str) -> anyhow::Result<User>;
    async fn list_users(&self) -> anyhow::Result<Vec<User>>;
}
