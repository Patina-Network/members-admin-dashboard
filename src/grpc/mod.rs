pub mod error;

use members_grpc_service::members::{
    self,
    admin::{self, members_admin_service_client::MembersAdminServiceClient},
    members_service_client::MembersServiceClient,
};
use std::{future::Future, time::Duration};
use tonic::transport::Channel;

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

pub async fn call_admin<M, T, F>(
    timeout: Duration,
    message: M,
    rpc: impl FnOnce(Request<M>) -> F,
) -> Result<T, UpstreamError>
where
    F: Future<Output = Result<Response<T>, Status>>,
{
    call(timeout, message, |mut request| {
        request.metadata_mut().insert(
            "profile",
            tonic::metadata::MetadataValue::from_static("core_admin"),
        );

        rpc(request)
    })
    .await
}

pub trait MembersGrpcClient: Clone + Send + Sync + 'static {
    fn get_member(
        &mut self,
        request: Request<members::GetMemberRequest>,
    ) -> impl Future<Output = Result<Response<members::GetMemberResponse>, Status>> + Send;

    fn list_members(
        &mut self,
        request: Request<members::ListMembersRequest>,
    ) -> impl Future<Output = Result<Response<members::ListMembersResponse>, Status>> + Send;

    fn list_careers(
        &mut self,
        request: Request<members::ListCareersRequest>,
    ) -> impl Future<Output = Result<Response<members::ListCareersResponse>, Status>> + Send;

    fn list_industries(
        &mut self,
        request: Request<members::ListIndustriesRequest>,
    ) -> impl Future<Output = Result<Response<members::ListIndustriesResponse>, Status>> + Send;
}

impl MembersGrpcClient for MembersServiceClient<Channel> {
    async fn get_member(
        &mut self,
        request: Request<members::GetMemberRequest>,
    ) -> Result<Response<members::GetMemberResponse>, Status> {
        MembersServiceClient::get_member(self, request).await
    }

    async fn list_members(
        &mut self,
        request: Request<members::ListMembersRequest>,
    ) -> Result<Response<members::ListMembersResponse>, Status> {
        MembersServiceClient::list_members(self, request).await
    }

    async fn list_careers(
        &mut self,
        request: Request<members::ListCareersRequest>,
    ) -> Result<Response<members::ListCareersResponse>, Status> {
        MembersServiceClient::list_careers(self, request).await
    }

    async fn list_industries(
        &mut self,
        request: Request<members::ListIndustriesRequest>,
    ) -> Result<Response<members::ListIndustriesResponse>, Status> {
        MembersServiceClient::list_industries(self, request).await
    }
}

pub trait MembersAdminGrpcClient: Clone + Send + Sync + 'static {
    fn get_sensitive_member(
        &mut self,
        request: Request<members::GetMemberRequest>,
    ) -> impl Future<Output = Result<Response<admin::GetSensitiveMemberResponse>, Status>> + Send;

    fn list_sensitive_members(
        &mut self,
        request: Request<members::ListMembersRequest>,
    ) -> impl Future<Output = Result<Response<admin::ListSensitiveMembersResponse>, Status>> + Send;

    fn create_member(
        &mut self,
        request: Request<admin::CreateMemberRequest>,
    ) -> impl Future<Output = Result<Response<admin::CreateMemberResponse>, Status>> + Send;

    fn update_member(
        &mut self,
        request: Request<admin::UpdateMemberRequest>,
    ) -> impl Future<Output = Result<Response<admin::UpdateMemberResponse>, Status>> + Send;

    fn bulk_member_operations(
        &mut self,
        request: Request<admin::BulkMemberOperationsRequest>,
    ) -> impl Future<Output = Result<Response<admin::BulkMemberOperationsResponse>, Status>> + Send;

    fn archive_member(
        &mut self,
        request: Request<admin::ArchiveMemberRequest>,
    ) -> impl Future<Output = Result<Response<admin::ArchiveMemberResponse>, Status>> + Send;
}

impl MembersAdminGrpcClient for MembersAdminServiceClient<Channel> {
    async fn get_sensitive_member(
        &mut self,
        request: Request<members::GetMemberRequest>,
    ) -> Result<Response<admin::GetSensitiveMemberResponse>, Status> {
        MembersAdminServiceClient::get_sensitive_member(self, request).await
    }

    async fn list_sensitive_members(
        &mut self,
        request: Request<members::ListMembersRequest>,
    ) -> Result<Response<admin::ListSensitiveMembersResponse>, Status> {
        MembersAdminServiceClient::list_sensitive_members(self, request).await
    }

    async fn create_member(
        &mut self,
        request: Request<admin::CreateMemberRequest>,
    ) -> Result<Response<admin::CreateMemberResponse>, Status> {
        MembersAdminServiceClient::create_member(self, request).await
    }

    async fn update_member(
        &mut self,
        request: Request<admin::UpdateMemberRequest>,
    ) -> Result<Response<admin::UpdateMemberResponse>, Status> {
        MembersAdminServiceClient::update_member(self, request).await
    }

    async fn bulk_member_operations(
        &mut self,
        request: Request<admin::BulkMemberOperationsRequest>,
    ) -> Result<Response<admin::BulkMemberOperationsResponse>, Status> {
        MembersAdminServiceClient::bulk_member_operations(self, request).await
    }

    async fn archive_member(
        &mut self,
        request: Request<admin::ArchiveMemberRequest>,
    ) -> Result<Response<admin::ArchiveMemberResponse>, Status> {
        MembersAdminServiceClient::archive_member(self, request).await
    }
}

#[cfg(test)]
pub(crate) mod fake {
    use std::{
        collections::VecDeque,
        sync::{Arc, Mutex},
    };

    use members_grpc_service::members::{
        self,
        admin::{
            ArchiveMemberRequest, ArchiveMemberResponse, BulkMemberOperationsRequest,
            BulkMemberOperationsResponse, CreateMemberRequest, CreateMemberResponse,
            GetSensitiveMemberResponse, ListSensitiveMembersResponse, UpdateMemberRequest,
            UpdateMemberResponse,
        },
    };
    use tonic::{Code, Request, Response, Status};

    use super::{MembersAdminGrpcClient, MembersGrpcClient};

    #[derive(Clone, Default)]
    pub(crate) enum Outcome<T> {
        #[default]
        Unexpected,
        Reply(T),
        Fail(Code),
        Hang,
    }

    async fn respond<T>(outcome: Outcome<T>, rpc: &'static str) -> Result<Response<T>, Status> {
        match outcome {
            Outcome::Unexpected => panic!("unexpected call to {rpc}"),
            Outcome::Reply(value) => Ok(Response::new(value)),
            Outcome::Fail(code) => Err(Status::new(code, "private upstream details")),
            Outcome::Hang => std::future::pending().await,
        }
    }

    fn assert_admin<M>(request: &Request<M>) {
        assert_eq!(request.metadata().get("profile").unwrap(), "core_admin");
        assert!(request.metadata().contains_key("grpc-timeout"));
    }

    pub(crate) fn sent<T: Clone>(calls: &Arc<Mutex<Vec<T>>>) -> Vec<T> {
        calls.lock().unwrap().clone()
    }

    /// Fake [`MembersAdminGrpcClient`]. Set an [`Outcome`] per RPC; every request is recorded.
    ///
    /// ```ignore
    /// let admin = FakeAdmin {
    ///     archive_response: Outcome::Reply(ArchiveMemberResponse { member: Some(member) }),
    ///     ..Default::default()
    /// };
    ///
    /// let mut client = admin.clone();
    /// call_admin(timeout, ArchiveMemberRequest { member_id: 42 }, |request| async move {
    ///     client.archive_member(request).await
    /// })
    /// .await
    /// .unwrap();
    ///
    /// assert_eq!(sent(&admin.archive_requests)[0].member_id, 42);
    /// ```
    #[derive(Clone, Default)]
    pub(crate) struct FakeAdmin {
        pub(crate) get_requests: Arc<Mutex<Vec<members::GetMemberRequest>>>,
        pub(crate) get_response: Outcome<GetSensitiveMemberResponse>,
        pub(crate) create_requests: Arc<Mutex<Vec<CreateMemberRequest>>>,
        pub(crate) create_response: Outcome<CreateMemberResponse>,
        pub(crate) update_requests: Arc<Mutex<Vec<UpdateMemberRequest>>>,
        pub(crate) update_response: Outcome<UpdateMemberResponse>,
        pub(crate) bulk_requests: Arc<Mutex<Vec<BulkMemberOperationsRequest>>>,
        pub(crate) bulk_response: Outcome<BulkMemberOperationsResponse>,
        pub(crate) archive_requests: Arc<Mutex<Vec<ArchiveMemberRequest>>>,
        pub(crate) archive_response: Outcome<ArchiveMemberResponse>,
    }

    impl MembersAdminGrpcClient for FakeAdmin {
        async fn get_sensitive_member(
            &mut self,
            request: Request<members::GetMemberRequest>,
        ) -> Result<Response<GetSensitiveMemberResponse>, Status> {
            assert_admin(&request);
            self.get_requests.lock().unwrap().push(request.into_inner());
            respond(self.get_response.clone(), "get_sensitive_member").await
        }

        async fn list_sensitive_members(
            &mut self,
            _: Request<members::ListMembersRequest>,
        ) -> Result<Response<ListSensitiveMembersResponse>, Status> {
            panic!("unexpected call to list_sensitive_members")
        }

        async fn create_member(
            &mut self,
            request: Request<CreateMemberRequest>,
        ) -> Result<Response<CreateMemberResponse>, Status> {
            assert_admin(&request);
            self.create_requests
                .lock()
                .unwrap()
                .push(request.into_inner());
            respond(self.create_response.clone(), "create_member").await
        }

        async fn update_member(
            &mut self,
            request: Request<UpdateMemberRequest>,
        ) -> Result<Response<UpdateMemberResponse>, Status> {
            assert_admin(&request);
            self.update_requests
                .lock()
                .unwrap()
                .push(request.into_inner());
            respond(self.update_response.clone(), "update_member").await
        }

        async fn bulk_member_operations(
            &mut self,
            request: Request<BulkMemberOperationsRequest>,
        ) -> Result<Response<BulkMemberOperationsResponse>, Status> {
            assert_admin(&request);
            self.bulk_requests
                .lock()
                .unwrap()
                .push(request.into_inner());
            respond(self.bulk_response.clone(), "bulk_member_operations").await
        }

        async fn archive_member(
            &mut self,
            request: Request<ArchiveMemberRequest>,
        ) -> Result<Response<ArchiveMemberResponse>, Status> {
            assert_admin(&request);
            self.archive_requests
                .lock()
                .unwrap()
                .push(request.into_inner());
            respond(self.archive_response.clone(), "archive_member").await
        }
    }

    /// Fake [`MembersGrpcClient`]. `list_members` returns queued pages in order; requests are recorded.
    ///
    /// ```ignore
    /// let members = FakeMembers::pages([Outcome::Fail(Code::Unavailable)]);
    ///
    /// let mut client = members.clone();
    /// let result = call(timeout, ListMembersRequest::default(), |request| async move {
    ///     client.list_members(request).await
    /// })
    /// .await;
    ///
    /// assert!(matches!(result, Err(UpstreamError::Unavailable)));
    /// assert_eq!(sent(&members.list_requests).len(), 1);
    /// ```
    #[derive(Clone, Default)]
    pub(crate) struct FakeMembers {
        pub(crate) list_requests: Arc<Mutex<Vec<members::ListMembersRequest>>>,
        pub(crate) list_responses: Arc<Mutex<VecDeque<Outcome<members::ListMembersResponse>>>>,
        pub(crate) careers_requests: Arc<Mutex<Vec<members::ListCareersRequest>>>,
        pub(crate) careers_response: Outcome<members::ListCareersResponse>,
        pub(crate) industries_requests: Arc<Mutex<Vec<members::ListIndustriesRequest>>>,
        pub(crate) industries_response: Outcome<members::ListIndustriesResponse>,
    }

    impl FakeMembers {
        pub(crate) fn pages(
            pages: impl IntoIterator<Item = Outcome<members::ListMembersResponse>>,
        ) -> Self {
            Self {
                list_responses: Arc::new(Mutex::new(pages.into_iter().collect())),
                ..Default::default()
            }
        }
    }

    impl MembersGrpcClient for FakeMembers {
        async fn get_member(
            &mut self,
            _: Request<members::GetMemberRequest>,
        ) -> Result<Response<members::GetMemberResponse>, Status> {
            panic!("unexpected call to get_member")
        }

        async fn list_members(
            &mut self,
            request: Request<members::ListMembersRequest>,
        ) -> Result<Response<members::ListMembersResponse>, Status> {
            assert!(request.metadata().get("profile").is_none());
            assert!(request.metadata().contains_key("grpc-timeout"));
            self.list_requests
                .lock()
                .unwrap()
                .push(request.into_inner());
            let page = self
                .list_responses
                .lock()
                .unwrap()
                .pop_front()
                .unwrap_or_default();
            respond(page, "list_members").await
        }

        async fn list_careers(
            &mut self,
            request: Request<members::ListCareersRequest>,
        ) -> Result<Response<members::ListCareersResponse>, Status> {
            assert!(request.metadata().contains_key("grpc-timeout"));
            self.careers_requests
                .lock()
                .unwrap()
                .push(request.into_inner());
            respond(self.careers_response.clone(), "list_careers").await
        }

        async fn list_industries(
            &mut self,
            request: Request<members::ListIndustriesRequest>,
        ) -> Result<Response<members::ListIndustriesResponse>, Status> {
            assert!(request.metadata().contains_key("grpc-timeout"));
            self.industries_requests
                .lock()
                .unwrap()
                .push(request.into_inner());
            respond(self.industries_response.clone(), "list_industries").await
        }
    }
}
