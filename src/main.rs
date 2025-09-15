mod config; mod ep; mod http; mod router; mod server; mod session; mod upload; mod util; mod cgi;

fn main() -> anyhow::Result<()> {
    let cfg = config::Config::load("localhost.json")?;
    let srv = server::HttpServer::new(cfg)?;
    eprintln!("Listening...");
    srv.run()?;
    Ok(())
}
