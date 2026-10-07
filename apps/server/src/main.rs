use actix_web::{middleware, web, App, HttpServer};
use std::io;

use llmtrack::app::{self, AppState};
use llmtrack::config::Config;
use llmtrack::routes::dashboard::Dashboard;
use llmtrack::{db, routes, services};

#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

fn startup_error(what: &str, e: impl std::fmt::Display) -> io::Error {
    log::error!("{what}: {e}");
    io::Error::other(format!("{what}: {e}"))
}

#[actix_web::main]
async fn main() -> io::Result<()> {
    dotenvy::dotenv().ok();
    env_logger::Builder::from_env(env_logger::Env::new().default_filter_or("info")).init();

    let config = Config::from_env().map_err(|e| startup_error("Configuration error", e))?;
    log::info!(
        "Starting llmtrack on {}:{}",
        config
            .host
            .as_deref()
            .unwrap_or("every interface (IPv6 and IPv4)"),
        config.port
    );

    let pool = db::create_pool(&config.database_url, config.max_connections)
        .await
        .map_err(|e| startup_error("Database error", e))?;
    db::run_migrations(&pool)
        .await
        .map_err(|e| startup_error("Migration error", e))?;
    services::users::ensure_superuser(&pool, std::env::var("CREATE_SUPERUSER").ok().as_deref())
        .await
        .map_err(|e| startup_error("CREATE_SUPERUSER", e))?;

    let key = config.master_key();
    let state = web::Data::new(
        AppState::new(pool, key.master(), config.upstream_timeout)
            .with_master_key(config.api_master_key.clone()),
    );
    services::catalog::load_stored(&state)
        .await
        .map_err(|e| startup_error("Price catalog", e))?;

    // Detected once, not per worker: the answer cannot change while the
    // process runs.
    let dashboard = config
        .dashboard_enabled
        .then(|| Dashboard::detect(&config.dashboard_dir))
        .flatten();
    match &dashboard {
        Some(found) => log::info!("Serving the dashboard from {}", found.root().display()),
        None => log::info!(
            "No dashboard at {} — serving the API only",
            config.dashboard_dir
        ),
    }

    let secure_cookies = config.ssl_proxy;
    let server_state = state.clone();
    let mut server = HttpServer::new(move || {
        let app = App::new()
            .app_data(server_state.clone())
            .wrap(middleware::Logger::new(&format!(
                "%a \"%r\" %s %b %T incident=%{{{}}}o",
                llmtrack::error::INCIDENT_ID_HEADER
            )))
            .wrap(middleware::Compress::default())
            .wrap(app::session_middleware(key.clone(), secure_cookies))
            .configure(app::configure);
        // The dashboard goes last: it ends in a catch-all that answers every
        // unclaimed path, so nothing registered after it would be reached.
        match &dashboard {
            Some(dashboard) => app.configure(dashboard.configure()),
            None => app.default_service(web::to(routes::not_found)),
        }
    })
    .shutdown_timeout(30);
    for socket in llmtrack::net::listeners(config.host.as_deref(), config.port)? {
        log::info!("Listening on {}", socket.local_addr()?);
        server = server.listen(socket)?;
    }
    let server = server.run();

    // Actix drains in-flight requests on SIGINT/SIGTERM before `run` resolves.
    server.await?;
    // Streams finished during the drain recorded their usage; write it before
    // the process goes.
    state.gateway.flush().await;
    Ok(())
}
