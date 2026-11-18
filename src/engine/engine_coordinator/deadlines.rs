use super::{timeout, Duration, EngineError};
use std::future::Future;

pub(super) async fn cdp_result<T, E>(
    deadline: Duration,
    operation: impl Future<Output = Result<T, E>>,
    rejection: EngineError,
) -> Result<T, EngineError> {
    match timeout(deadline, operation).await {
        Ok(Ok(value)) => Ok(value),
        Ok(Err(_)) => Err(rejection),
        Err(_) => Err(EngineError::Timeout),
    }
}
