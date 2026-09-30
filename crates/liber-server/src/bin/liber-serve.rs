#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let mut addr = "127.0.0.1:8080".to_string();
    let mut token_flag = String::new();
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
            other => return Err(anyhow::anyhow!("unknown flag {other:?}")),
        }
    }
    let (cfg, _) = liber_core::config::load_config()?;
    let token = cfg.resolve_auth_token(&token_flag);
    liber_server::serve(cfg, token, &addr).await
}
