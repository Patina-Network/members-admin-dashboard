use std::time::Duration;

use anyhow::Context;
use axum::{
    extract::MatchedPath,
    http::{Method, StatusCode},
};
use metrics_exporter_prometheus::{PrometheusBuilder, PrometheusHandle};
use strum::AsRefStr;

use crate::grpc::error::UpstreamError;

const HTTP_REQUEST_DURATION: &str = "http_server_request_duration_seconds";
const GRPC_CLIENT_REQUEST_DURATION: &str = "grpc_client_request_duration_seconds";
const AUTH_LOGINS: &str = "auth_logins_total";

const LATENCY_BUCKETS: &[f64] = &[
    0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0,
];

/// Outbound gRPC method, used as the `method` label.
#[derive(Clone, Copy, AsRefStr)]
pub enum GrpcMethod {
    ListMembers,
    ListCareers,
    ListIndustries,
    GetSensitiveMember,
    CreateMember,
    UpdateMember,
    ArchiveMember,
    BulkMemberOperations,
}

/// Result of an Azure login attempt, used as the `outcome` label.
#[derive(Clone, Copy, AsRefStr)]
#[strum(serialize_all = "snake_case")]
pub enum LoginOutcome {
    Success,
    Forbidden,
    Cancelled,
    InvalidCallback,
    Failed,
    Unavailable,
}

pub fn install_recorder() -> anyhow::Result<PrometheusHandle> {
    PrometheusBuilder::new()
        .set_buckets(LATENCY_BUCKETS)
        .context("invalid histogram buckets")?
        .install_recorder()
        .context("failed to install Prometheus recorder")
}

/// `route` is `None` when no route matched (SPA fallback).
pub fn record_http_request(
    method: &Method,
    route: Option<&MatchedPath>,
    status: StatusCode,
    elapsed: Duration,
) {
    ::metrics::histogram!(
        HTTP_REQUEST_DURATION,
        "method" => method.as_str().to_owned(),
        "route" => route.map_or("unmatched", MatchedPath::as_str).to_owned(),
        "status" => status.as_str().to_owned(),
    )
    .record(elapsed);
}

pub fn record_grpc_request(
    method: GrpcMethod,
    result: Result<(), &UpstreamError>,
    elapsed: Duration,
) {
    let code = match result {
        Ok(()) => "Ok",
        Err(error) => error.as_ref(),
    };

    ::metrics::histogram!(
        GRPC_CLIENT_REQUEST_DURATION,
        "method" => method.as_ref().to_owned(),
        "code" => code.to_owned(),
    )
    .record(elapsed);
}

pub fn record_login(outcome: LoginOutcome) {
    ::metrics::counter!(AUTH_LOGINS, "outcome" => outcome.as_ref().to_owned()).increment(1);
}
