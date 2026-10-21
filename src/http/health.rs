/// Readiness: Kubernetes removes an unready pod from service endpoints.
/// This scaffold reports process availability, not upstream service health.
pub async fn ready() -> &'static str {
    "ok"
}

/// Liveness: Kubernetes restarts the container if this probe fails.
/// Keep external service calls out of liveness checks.
pub async fn live() -> &'static str {
    "ok"
}
