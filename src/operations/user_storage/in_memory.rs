use std::{collections::HashMap, fmt::Debug, sync::Arc};

use serde::Serialize;
use tokio::sync::RwLock;

use crate::models::User;

use super::UserStorage;

#[derive(Clone)]
pub struct InMemoryUserStorage {
    users: Arc<RwLock<HashMap<String, User>>>,
}

impl InMemoryUserStorage {
    fn new() -> Self {
        Self {
            users: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

impl Default for InMemoryUserStorage {
    fn default() -> Self {
        Self::new()
    }
}

impl Debug for InMemoryUserStorage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut formatter = f.debug_struct("InMemoryUserStorage");
        if let Ok(users) = self.users.try_read() {
            formatter.field("users", &users);
        }
        formatter.finish()
    }
}

#[async_trait::async_trait]
impl UserStorage for InMemoryUserStorage {
    async fn register_user<U: Into<User> + Serialize + Send + Sync>(
        &self,
        user: U,
    ) -> anyhow::Result<User> {
        let user = user.into();
        let mut users = self.users.write().await;
        users.insert(user.name.clone(), user.clone());
        Ok(user)
    }

    async fn get_user(&self, username: &str) -> anyhow::Result<User> {
        let users = self.users.read().await;
        users
            .get(username)
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("no such user"))
    }

    async fn list_users(&self) -> anyhow::Result<Vec<User>> {
        todo!()
    }
}
