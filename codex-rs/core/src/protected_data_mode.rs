use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use codex_protocol::ThreadId;
use tokio::sync::RwLock;

pub type ProtectedDataModeExitFuture<'a> =
    Pin<Box<dyn Future<Output = anyhow::Result<bool>> + Send + 'a>>;

pub trait ProtectedDataModeExitPolicy: Send + Sync {
    fn can_exit(&self, thread_id: ThreadId) -> ProtectedDataModeExitFuture<'_>;
}

#[derive(Default)]
pub struct DenyProtectedDataModeExitPolicy;

impl ProtectedDataModeExitPolicy for DenyProtectedDataModeExitPolicy {
    fn can_exit(&self, _thread_id: ThreadId) -> ProtectedDataModeExitFuture<'_> {
        Box::pin(async { Ok(false) })
    }
}

pub(crate) fn default_exit_policy() -> Arc<RwLock<Arc<dyn ProtectedDataModeExitPolicy>>> {
    Arc::new(RwLock::new(Arc::new(DenyProtectedDataModeExitPolicy)))
}
