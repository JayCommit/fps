pub mod error;
pub mod extractors;
pub mod routes;

use axum::extract::{DefaultBodyLimit, FromRequestParts, Request, State};
use axum::http::{HeaderValue, Method};
use axum::middleware::{self, Next};
use axum::response::Response;
use axum::routing::{get, patch, post};
use axum::Router;
use tower::ServiceBuilder;
use tower_http::cors::{AllowOrigin, CorsLayer};
use tower_http::request_id::{MakeRequestUuid, PropagateRequestIdLayer, SetRequestIdLayer};
use tower_http::set_header::SetResponseHeaderLayer;
use tower_http::trace::TraceLayer;
use utoipa::OpenApi;

use crate::state::AppState;

use self::extractors::AuthUser;
use self::routes::{
    audit, auth, dashboard, health, invitations, nodes, notifications, servers, setup, templates,
    users,
};

const REQUEST_ID_HEADER: &str = "x-request-id";

#[derive(OpenApi)]
#[openapi(
    info(
        title = "FPS Control Plane",
        version = "0.0.1-alpha.1",
        description = "Canonical HTTP contract for the control plane. Generated from source."
    ),
    paths(
        health::health,
        health::ready,
        health::version,
        setup::setup_status,
        setup::complete_setup,
        auth::login,
        auth::logout,
        auth::refresh,
        auth::me,
        auth::enable_totp,
        auth::confirm_totp,
        nodes::list_nodes,
        nodes::get_node,
        nodes::create_enrollment_token,
        nodes::enroll,
        nodes::heartbeat,
        dashboard::summary,
        users::list_users,
        users::patch_user,
        invitations::list_invitations,
        invitations::create_invitation,
        invitations::accept_invitation,
        audit::list_audit,
        templates::list_templates,
        templates::create_template,
        templates::import_egg_template,
        servers::list_servers,
        servers::get_server,
        servers::create_server,
        servers::start_server,
        servers::stop_server,
        servers::backup_server,
        servers::server_logs,
        servers::list_backups,
        servers::list_schedules,
        servers::create_schedule,
        notifications::list_notifications,
        notifications::read_notification,
        nodes::revoke_node,
    ),
    components(schemas(
        fps_domain::user::UserSummary,
        fps_domain::roles::Role,
        fps_domain::node::NodeHealth,
        fps_domain::node::NodeStatus,
        fps_domain::node::DockerState,
        fps_protocol::EnrollRequest,
        fps_protocol::EnrollResponse,
        fps_protocol::HeartbeatRequest,
        fps_protocol::HeartbeatResponse,
        fps_protocol::DockerCapability,
        setup::SetupRequest,
        setup::SetupStatus,
        auth::LoginRequest,
        auth::LoginResponse,
        auth::MeResponse,
        auth::TotpStartResponse,
        auth::TotpConfirmRequest,
        auth::TotpConfirmResponse,
        nodes::EnrollmentTokenRequest,
        nodes::EnrollmentTokenResponse,
        nodes::NodeView,
        dashboard::DashboardSummary,
        users::PatchUserRequest,
        invitations::InvitationView,
        invitations::CreateInvitationRequest,
        invitations::AcceptInvitationRequest,
        audit::AuditEventView,
        servers::CreateServerRequest,
        servers::ServerDetail,
        servers::LogLine,
        servers::ScheduleView,
        servers::CreateScheduleRequest,
        notifications::NotificationView,
        fps_domain::server::ServerSummary,
        fps_domain::template::TemplateSummary,
        fps_domain::backup::BackupSummary,
        fps_protocol::JobInstruction,
        fps_protocol::JobResult,
        error::Problem
    )),
    tags(
        (name = "health", description = "Liveness and version"),
        (name = "setup", description = "First-run owner creation"),
        (name = "auth", description = "Sessions and MFA"),
        (name = "nodes", description = "Game node enrollment and health"),
        (name = "dashboard", description = "Aggregated operational view"),
        (name = "identity", description = "Users, invitations, and audit"),
        (name = "servers", description = "Game servers, jobs, and schedules"),
        (name = "templates", description = "Native templates and Egg import"),
        (name = "backups", description = "Backup inventory"),
        (name = "ops", description = "In-app notifications")
    )
)]
pub struct ApiDoc;

pub fn openapi_json() -> String {
    ApiDoc::openapi().to_pretty_json().expect("openapi")
}

async fn protect_docs_and_metrics(
    State(state): State<AppState>,
    request: Request,
    next: Next,
) -> Result<Response, error::ApiError> {
    let path = request.uri().path().to_string();
    if path == "/metrics" || path == "/openapi.json" || path.starts_with("/docs") {
        let (mut parts, body) = request.into_parts();
        let auth = AuthUser::from_request_parts(&mut parts, &state).await?;
        auth.require(fps_domain::Permission::DiagnosticsRead)?;
        let request = Request::from_parts(parts, body);
        return Ok(next.run(request).await);
    }
    Ok(next.run(request).await)
}

pub fn router(state: AppState) -> Router {
    let origins = state.config.cors_origins.clone();
    let cors = CorsLayer::new()
        .allow_origin(AllowOrigin::list(
            origins
                .into_iter()
                .filter_map(|o| o.parse::<HeaderValue>().ok())
                .collect::<Vec<_>>(),
        ))
        .allow_methods([
            Method::GET,
            Method::POST,
            Method::DELETE,
            Method::PATCH,
            Method::PUT,
            Method::OPTIONS,
        ])
        .allow_headers([
            axum::http::header::AUTHORIZATION,
            axum::http::header::CONTENT_TYPE,
            axum::http::header::COOKIE,
            axum::http::header::HeaderName::from_static("x-csrf-token"),
            axum::http::header::HeaderName::from_static("idempotency-key"),
            axum::http::header::HeaderName::from_static("x-request-id"),
        ])
        .allow_credentials(true);

    let csp = HeaderValue::from_static(
        "default-src 'self'; img-src 'self' data:; style-src 'self' 'unsafe-inline'; script-src 'self'; connect-src 'self' ws: wss:; frame-ancestors 'none'; base-uri 'self'; form-action 'self'",
    );

    Router::new()
        .route("/health", get(health::health))
        .route("/ready", get(health::ready))
        .route("/version", get(health::version))
        .route("/metrics", get(health::metrics))
        .merge(utoipa_swagger_ui::SwaggerUi::new("/docs").url("/openapi.json", ApiDoc::openapi()))
        .route("/v1/setup/status", get(setup::setup_status))
        .route("/v1/setup", post(setup::complete_setup))
        .route("/v1/auth/login", post(auth::login))
        .route("/v1/auth/logout", post(auth::logout))
        .route("/v1/auth/refresh", post(auth::refresh))
        .route("/v1/auth/me", get(auth::me))
        .route("/v1/auth/totp/start", post(auth::enable_totp))
        .route("/v1/auth/totp/confirm", post(auth::confirm_totp))
        .route("/v1/users", get(users::list_users))
        .route("/v1/users/{id}", patch(users::patch_user))
        .route(
            "/v1/invitations",
            get(invitations::list_invitations).post(invitations::create_invitation),
        )
        .route(
            "/v1/invitations/accept",
            post(invitations::accept_invitation),
        )
        .route("/v1/audit", get(audit::list_audit))
        .route(
            "/v1/templates",
            get(templates::list_templates).post(templates::create_template),
        )
        .route(
            "/v1/templates/import-egg",
            post(templates::import_egg_template),
        )
        .route(
            "/v1/servers",
            get(servers::list_servers).post(servers::create_server),
        )
        .route("/v1/servers/{id}/start", post(servers::start_server))
        .route("/v1/servers/{id}/stop", post(servers::stop_server))
        .route("/v1/servers/{id}/backup", post(servers::backup_server))
        .route("/v1/servers/{id}/logs", get(servers::server_logs))
        .route(
            "/v1/servers/{id}/files/refresh",
            post(servers::refresh_files),
        )
        .route("/v1/servers/{id}/files", get(servers::server_files))
        .route("/v1/servers/{id}", get(servers::get_server))
        .route("/v1/backups", get(servers::list_backups))
        .route(
            "/v1/schedules",
            get(servers::list_schedules).post(servers::create_schedule),
        )
        .route("/v1/schedules/{id}", patch(servers::patch_schedule))
        .route("/v1/notifications", get(notifications::list_notifications))
        .route(
            "/v1/notifications/{id}/read",
            post(notifications::read_notification),
        )
        .route("/v1/nodes", get(nodes::list_nodes))
        .route(
            "/v1/nodes/enrollment-tokens",
            post(nodes::create_enrollment_token),
        )
        .route("/v1/nodes/enroll", post(nodes::enroll))
        .route("/v1/nodes/{id}/revoke", post(nodes::revoke_node))
        .route("/v1/nodes/{id}/heartbeat", post(nodes::heartbeat))
        .route("/v1/nodes/{id}", get(nodes::get_node))
        .route("/v1/dashboard", get(dashboard::summary))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            protect_docs_and_metrics,
        ))
        .layer(DefaultBodyLimit::max(1024 * 1024))
        .layer(
            ServiceBuilder::new()
                .layer(SetRequestIdLayer::new(
                    axum::http::header::HeaderName::from_static(REQUEST_ID_HEADER),
                    MakeRequestUuid,
                ))
                .layer(PropagateRequestIdLayer::new(
                    axum::http::header::HeaderName::from_static(REQUEST_ID_HEADER),
                ))
                .layer(TraceLayer::new_for_http())
                .layer(SetResponseHeaderLayer::overriding(
                    axum::http::header::HeaderName::from_static("content-security-policy"),
                    csp,
                ))
                .layer(SetResponseHeaderLayer::if_not_present(
                    axum::http::header::HeaderName::from_static("x-content-type-options"),
                    HeaderValue::from_static("nosniff"),
                ))
                .layer(SetResponseHeaderLayer::if_not_present(
                    axum::http::header::HeaderName::from_static("referrer-policy"),
                    HeaderValue::from_static("no-referrer"),
                ))
                .layer(SetResponseHeaderLayer::if_not_present(
                    axum::http::header::HeaderName::from_static("x-frame-options"),
                    HeaderValue::from_static("DENY"),
                ))
                .layer(cors),
        )
        .with_state(state)
}

/// Node mTLS listener: heartbeat only. Identity is the client certificate.
pub fn node_router(state: AppState) -> Router {
    Router::new()
        .route("/v1/nodes/{id}/heartbeat", post(nodes::heartbeat_mtls))
        .route("/health", get(health::health))
        .with_state(state)
}
