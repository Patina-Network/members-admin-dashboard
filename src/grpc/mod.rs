pub mod error;

use std::{future::Future, time::Duration};

use tonic::{Request, Response, Status};

use error::UpstreamError;

/// use this function to make any outbound grpc requests.
pub async fn call<M, T, F>(
    timeout: Duration,
    message: M,
    rpc: impl FnOnce(Request<M>) -> F,
) -> Result<T, UpstreamError>
where
    F: Future<Output = Result<Response<T>, Status>>,
{
    let mut request = Request::new(message);
    request.set_timeout(timeout);
    tokio::time::timeout(timeout, rpc(request))
        .await
        .map_err(|_| UpstreamError::Timeout)?
        .map(Response::into_inner)
        .map_err(Into::into)
}
