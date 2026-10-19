pub mod agent;
pub mod ai_review;
pub mod ai_usage;
pub mod auth;
pub mod cache;
pub mod comments;
pub mod demo;
pub mod diff;
pub mod files;
pub mod focus;
pub mod keyring;
pub mod oauth;
pub mod pr_filters;
pub mod pr_list_cache;
pub mod provider;
pub mod rank_findings;
pub mod semantic;
pub mod source_snapshot;
pub mod time;

use cache::Cache;
use provider::GitProvider;
use std::sync::{atomic::AtomicBool, Arc};
use tokio::sync::{Mutex, RwLock, RwLockReadGuard};

pub struct PendingLogin {
    pub login: oauth::DeviceLogin,
    pub cancelled: Arc<AtomicBool>,
    pub polling: bool,
}

pub struct AppState {
    pub provider: RwLock<Option<Box<dyn GitProvider>>>,
    pub cache: Mutex<Cache>,
    pub pending_login: Mutex<Option<PendingLogin>>,
}

impl AppState {
    pub fn open() -> anyhow::Result<Self> {
        let cache = Cache::open()?;
        // A sample left open when the app quit.
        demo::clear(cache.conn())?;
        Ok(Self {
            provider: RwLock::new(None),
            cache: Mutex::new(cache),
            pending_login: Mutex::new(None),
        })
    }

    /// Read access to the signed-in provider.
    pub async fn provider(&self) -> Result<RwLockReadGuard<'_, dyn GitProvider>, String> {
        RwLockReadGuard::try_map(self.provider.read().await, |p| p.as_deref())
            .map_err(|_| "Not authenticated".to_string())
    }
}
