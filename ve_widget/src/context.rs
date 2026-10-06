use std::{any::Any, sync::Arc};
use tokio::sync::RwLock;

#[derive(Clone)]
pub struct Context {
    pub user: Arc<RwLock<Box<dyn Any + Send + Sync>>>,
}

impl Context {
    pub fn new(user: Box<dyn Any + Send + Sync>) -> Self {
        Self {
            user: Arc::new(RwLock::new(user)),
        }
    }
}
