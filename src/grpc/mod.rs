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

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn call_sets_deadline_and_returns_response_body() {
        let result = call(Duration::from_secs(1), "input", |request| async move {
            assert!(request.metadata().contains_key("grpc-timeout"));
            assert_eq!(request.into_inner(), "input");
            Ok(Response::new("output"))
        })
        .await
        .unwrap();
        assert_eq!(result, "output");
    }

    #[tokio::test]
    async fn call_times_out_even_when_upstream_does_not_respond() {
        let result = call(Duration::from_millis(1), (), |_| {
            std::future::pending::<Result<Response<()>, Status>>()
        })
        .await;
        assert!(matches!(result, Err(UpstreamError::Timeout)));
    }
}
