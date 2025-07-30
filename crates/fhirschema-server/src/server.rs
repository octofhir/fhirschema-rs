//! Main server implementation

use axum::{
    extract::DefaultBodyLimit,
    http::{header, Method},
    middleware,
    routing::get,
    Router,
};
use std::{net::SocketAddr, sync::Arc, time::Duration};
use tokio::signal;
use tower::{ServiceBuilder, make::Shared};
use tower_http::{
    compression::CompressionLayer,
    cors::{Any, CorsLayer},
    timeout::TimeoutLayer,
    trace::TraceLayer,
};
use tracing::{info, warn};

use crate::{
    config::ServerConfig,
    error::{Result, ServerError},
    middleware::{auth::AuthLayer, rate_limit::RateLimitLayer},
    routes,
    services::AppState,
};

/// FHIRSchema HTTP Server
pub struct Server {
    config: ServerConfig,
    app_state: Arc<AppState>,
}

impl Server {
    /// Create a new server instance
    pub async fn new(config: ServerConfig) -> Result<Self> {
        // Initialize application state
        let app_state = Arc::new(AppState::new(&config).await?);

        Ok(Self { config, app_state })
    }

    /// Start the server
    pub async fn start(self) -> Result<()> {
        let app = self.create_app().await?;
        let addr = self.get_socket_addr();

        info!("Starting FHIRSchema server on {}", addr);

        // Create server with graceful shutdown
        let listener = tokio::net::TcpListener::bind(&addr).await?;

        axum::serve(listener, app.into_make_service())
            .with_graceful_shutdown(shutdown_signal())
            .await
            .map_err(|e| ServerError::Internal(format!("Server error: {}", e)))?;

        info!("Server stopped gracefully");
        Ok(())
    }

    /// Create the Axum application
    async fn create_app(&self) -> Result<Router> {
        let app = Router::new()
            // Health check endpoint
            .route("/health", get(routes::health::health_check))
            // API routes
            .nest("/api/v1", self.create_api_routes())
            // Metrics endpoint (if enabled)
            .merge(self.create_metrics_routes())
            // Add application state
            .with_state(self.app_state.clone());

        // Add middleware layers after state
        let app = self.add_middleware_layers(app)?;

        Ok(app)
    }

    /// Create API routes
    fn create_api_routes(&self) -> Router {
        Router::new()
            // Validation endpoints
            .nest("/validate", routes::validation::create_routes())
            // Conversion endpoints
            .nest("/convert", routes::conversion::create_routes())
            // Schema repository endpoints
            .nest("/schemas", routes::schemas::create_routes())
            // IG processing endpoints
            .nest("/ig", routes::ig::create_routes())
            // Job management endpoints
            .nest("/jobs", routes::jobs::create_routes())
            // Server info endpoint
            .route("/info", get(routes::info::server_info))
    }

    /// Create metrics routes if enabled
    fn create_metrics_routes(&self) -> Router {
        if self.config.monitoring.metrics_enabled {
            Router::new().route(
                &self.config.monitoring.metrics_path,
                get(routes::metrics::metrics_handler),
            )
        } else {
            Router::new()
        }
    }

    /// Add middleware layers to the application
    fn add_middleware_layers(&self, mut app: Router<Arc<AppState>>) -> Result<Router> {
        // Set body size limit
        app = app.layer(DefaultBodyLimit::max(self.config.server.max_body_size));

        // Add timeout layer
        app = app.layer(TimeoutLayer::new(Duration::from_secs(
            self.config.server.timeout,
        )));

        // Add compression layer if enabled
        if self.config.server.compression_enabled {
            app = app.layer(CompressionLayer::new());
        }

        // Add CORS layer if enabled
        if self.config.server.cors_enabled {
            let cors = if self.config.server.cors_origins.contains(&"*".to_string()) {
                CorsLayer::new()
                    .allow_origin(Any)
                    .allow_methods([Method::GET, Method::POST, Method::PUT, Method::DELETE])
                    .allow_headers([header::CONTENT_TYPE, header::AUTHORIZATION])
            } else {
                CorsLayer::new()
                    .allow_origin(Any) // Simplified for now
                    .allow_methods([Method::GET, Method::POST, Method::PUT, Method::DELETE])
                    .allow_headers([header::CONTENT_TYPE, header::AUTHORIZATION])
            };
            app = app.layer(cors);
        }

        // Add tracing layer
        app = app.layer(TraceLayer::new_for_http());

        // Add rate limiting if enabled (placeholder)
        if self.config.auth.rate_limit.enabled {
            app = app.layer(RateLimitLayer::new(&self.config.auth.rate_limit));
        }

        // Add authentication layer if enabled (placeholder)
        if self.config.auth.enabled {
            app = app.layer(AuthLayer::new(&self.config.auth));
        }

        Ok(app)
    }

    /// Get the socket address for the server
    fn get_socket_addr(&self) -> SocketAddr {
        format!("{}:{}", self.config.server.host, self.config.server.port)
            .parse()
            .expect("Invalid server address")
    }
}

/// Graceful shutdown signal handler
async fn shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("Failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("Failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {
            info!("Received Ctrl+C signal");
        },
        _ = terminate => {
            info!("Received terminate signal");
        },
    }

    warn!("Starting graceful shutdown...");
}
