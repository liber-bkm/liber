#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let mut addr = "127.0.0.1:8080".to_string();
    let mut token_flag = String::new();
    let mut static_dir: Option<std::path::PathBuf> = None;
    let mut qr = false;
    let mut mdns = true;
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--addr" => {
                addr = args
                    .next()
                    .ok_or_else(|| anyhow::anyhow!("--addr needs a value"))?;
            }
            "--auth-token" => {
                token_flag = args
                    .next()
                    .ok_or_else(|| anyhow::anyhow!("--auth-token needs a value"))?;
            }
            "--static-dir" => {
                static_dir = Some(
                    args.next()
                        .ok_or_else(|| anyhow::anyhow!("--static-dir needs a value"))?
                        .into(),
                );
            }
            "--qr" => qr = true,
            "--no-mdns" => mdns = false,
            other => return Err(anyhow::anyhow!("unknown flag {other:?}")),
        }
    }
    let (cfg, _) = liber_core::config::load_config()?;
    let token = cfg.resolve_auth_token(&token_flag);
    liber_server::serve(
        cfg,
        token,
        &addr,
        static_dir,
        liber_server::ServeOptions { qr, mdns },
    )
    .await
}
