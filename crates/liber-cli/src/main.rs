use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "liber", version, about = "Local first bookmark manager")]
struct Cli {
    #[command(subcommand)]
    cmd: Option<Cmd>,

    #[arg(short, long)]
    list: bool,

    #[arg(short, long)]
    serve: bool,

    #[arg(long)]
    addr: Option<String>,

    #[arg(long)]
    auth_token: Option<String>,
}

#[derive(Subcommand)]
enum Cmd {
    Add(AddArgs),
    List(ListArgs),
    Open(OpenArgs),
    Edit(EditArgs),
    Delete(DeleteArgs),
    Reindex(ReindexArgs),
    Export(ExportArgs),
    Import(ImportArgs),
    Tags(TagsArgs),
    Folders(FoldersArgs),
    Auto(AutoArgs),
    History(HistoryArgs),
    Check(CheckArgs),
    Sync(SyncArgs),
    Config(ConfigArgs),
    Pick(PickArgs),
    Profile(ProfileArgs),
    Serve(ServeArgs),
    Completion(CompletionArgs),
}

#[derive(clap::Args)]
struct AddArgs {
    url: String,
    #[arg(long)]
    title: Option<String>,
    #[arg(short, long)]
    tags: Vec<String>,
    #[arg(short, long)]
    folder: Option<String>,
    #[arg(long)]
    description: Option<String>,
    #[arg(long)]
    markdown: bool,
    #[arg(long)]
    archive: bool,
}

#[derive(clap::Args)]
struct ListArgs {
    query: Option<String>,
    #[arg(long)]
    sort: Option<String>,
}

#[derive(clap::Args)]
struct OpenArgs {
    spec: String,
}

#[derive(clap::Args)]
struct EditArgs {
    spec: String,
    #[arg(short, long)]
    url: Option<String>,
    #[arg(short, long)]
    title: Option<String>,
    #[arg(short, long)]
    tags: Vec<String>,
    #[arg(short, long)]
    folder: Option<String>,
}

#[derive(clap::Args)]
struct DeleteArgs {
    spec: String,
    #[arg(long)]
    yes: bool,
}

#[derive(clap::Args)]
struct ReindexArgs {
    #[arg(long)]
    merge: bool,
    #[arg(long)]
    prune: bool,
    #[arg(long)]
    compact: bool,
}

#[derive(clap::Args)]
struct ExportArgs {
    #[arg(long)]
    site: bool,
    #[arg(long)]
    bookmarks: bool,
    out: Option<String>,
}

#[derive(clap::Args)]
struct ImportArgs {
    file: String,
    #[arg(long)]
    from_go: bool,
}

#[derive(clap::Args)]
struct TagsArgs {
    #[command(subcommand)]
    cmd: Option<TagsCmd>,
}

#[derive(Subcommand)]
enum TagsCmd {
    List,
    Rename { from: String, to: String },
    Delete { name: String },
}

#[derive(clap::Args)]
struct FoldersArgs {
    #[command(subcommand)]
    cmd: Option<FoldersCmd>,
}

#[derive(Subcommand)]
enum FoldersCmd {
    List,
    Rename { from: String, to: String },
    Delete { name: String },
}

#[derive(clap::Args)]
struct AutoArgs {
    #[command(subcommand)]
    cmd: AutoCmd,
}

#[derive(Subcommand)]
enum AutoCmd {
    Add { pattern: String },
    List,
    Apply,
    Learn,
    Delete { id: String },
}

#[derive(clap::Args)]
struct HistoryArgs {}

#[derive(clap::Args)]
struct CheckArgs {
    spec: Option<String>,
    #[arg(long)]
    apply: bool,
}

#[derive(clap::Args)]
struct SyncArgs {
    #[arg(long)]
    merge: bool,
    target: Option<String>,
}

#[derive(clap::Args)]
struct ConfigArgs {
    #[command(subcommand)]
    cmd: ConfigCmd,
}

#[derive(Subcommand)]
enum ConfigCmd {
    Get { key: String },
    Set { key: String, value: String },
    List,
}

#[derive(clap::Args)]
struct PickArgs {
    query: Option<String>,
}

#[derive(clap::Args)]
struct ProfileArgs {
    #[command(subcommand)]
    cmd: Option<ProfileCmd>,
}

#[derive(Subcommand)]
enum ProfileCmd {
    List,
    Switch { name: String },
}

#[derive(clap::Args)]
struct ServeArgs {
    #[arg(long, default_value = "127.0.0.1:8080")]
    addr: String,
    #[arg(long)]
    auth_token: Option<String>,
}

#[derive(clap::Args)]
struct CompletionArgs {
    shell: String,
}

fn load_store() -> anyhow::Result<(liber_core::store::Config, liber_core::store::Store)> {
    let (cfg, _) = liber_core::config::load_config()?;
    let store = liber_core::store::Store::open(cfg.clone())?;
    Ok((cfg, store))
}

fn display_folder(f: &str) -> &str {
    if f.is_empty() {
        "/"
    } else {
        f
    }
}

fn confirm(prompt: &str) -> bool {
    use std::io::{self, Write};
    print!("{prompt} [y/N] ");
    let _ = io::stdout().flush();
    let mut line = String::new();
    if io::stdin().read_line(&mut line).is_err() {
        return false;
    }
    matches!(line.trim().to_lowercase().as_str(), "y" | "yes")
}

fn open_in_browser(cfg: &liber_core::store::Config, url: &str) -> anyhow::Result<()> {
    println!("{url}");
    let mut cmd = if cfg.browser_cmd.trim().is_empty() {
        #[cfg(target_os = "macos")]
        let c = std::process::Command::new("open");
        #[cfg(target_os = "windows")]
        let c = {
            let mut c = std::process::Command::new("cmd");
            c.args(["/C", "start", ""]);
            c
        };
        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        let c = std::process::Command::new("xdg-open");
        c
    } else {
        let parts: Vec<&str> = cfg.browser_cmd.split_whitespace().collect();
        let mut c = std::process::Command::new(parts[0]);
        c.args(&parts[1..]);
        c
    };
    cmd.arg(url).status()?;
    Ok(())
}

fn run_add(a: AddArgs) -> anyhow::Result<()> {
    let (_, mut store) = load_store()?;
    let opts = liber_core::create::CreateOptions {
        title: a.title,
        description: a.description.unwrap_or_default(),
        tags: a.tags,
        folder: a.folder.unwrap_or_default(),
        markdown: a.markdown,
    };
    match liber_core::create::create_bookmark(&mut store, &a.url, opts) {
        Err(liber_core::CoreError::Duplicate(_)) => {
            let dup = store
                .find_by_url(&liber_core::slug::normalize_url(&a.url))?
                .unwrap();
            println!(
                "Already bookmarked: [{}] {} (folder: {})",
                &dup.uuid.to_string()[..8],
                dup.title,
                display_folder(&dup.folder)
            );
            if !confirm("Add it anyway?") {
                println!("Cancelled.");
                return Ok(());
            }
            println!("Use a distinct URL to add a second copy.");
            Ok(())
        }
        Err(e) => Err(e.into()),
        Ok(b) => {
            println!("\nSaved [{}] {}", &b.uuid.to_string()[..8], b.title);
            println!(
                "  html: {}",
                store.cfg.html_dir().join(&b.html_file).display()
            );
            if let Some(rel) = &b.markdown_file {
                println!(
                    "  markdown: {}",
                    store.cfg.markdown_dir().join(rel).display()
                );
            }
            Ok(())
        }
    }
}

fn run_list(a: ListArgs) -> anyhow::Result<()> {
    let (_, store) = load_store()?;
    let mut found = store.list()?;
    if let Some(q) = &a.query {
        let fields = liber_core::search::SearchFields::all();
        found.retain(|b| liber_core::search::bookmark_matches(b, q, &fields));
    }
    let sort = liber_core::search::parse_sort_mode(a.sort.as_deref().unwrap_or(""))?;
    let fields = liber_core::search::SearchFields::all();
    let query = a.query.unwrap_or_default();
    let ordered = liber_core::search::order_results(found, &query, &fields, sort);
    for b in ordered {
        println!(
            "[{}] {} ({}) {}",
            &b.uuid.to_string()[..8],
            b.title,
            display_folder(&b.folder),
            b.url
        );
    }
    Ok(())
}

fn run_open(spec: &str) -> anyhow::Result<()> {
    let (cfg, mut store) = load_store()?;
    let tokens = liber_core::idspec::parse_id_spec(spec)?;
    let found = match store.resolve_spec(&tokens) {
        Ok(v) => v,
        Err(_) if liber_core::idspec::is_query_spec(spec) => {
            let fields = liber_core::search::SearchFields::all();
            store
                .list()?
                .into_iter()
                .filter(|b| liber_core::search::bookmark_matches(b, spec, &fields))
                .collect()
        }
        Err(e) => return Err(e.into()),
    };
    if found.is_empty() {
        return Err(anyhow::anyhow!("no bookmarks matching {spec:?}"));
    }
    for b in found {
        store.record_open(&b.uuid)?;
        open_in_browser(&cfg, &b.url)?;
    }
    Ok(())
}

fn run_config(cmd: ConfigCmd) -> anyhow::Result<()> {
    let (mut cfg, path) = liber_core::config::load_config()?;
    match cmd {
        ConfigCmd::Get { key } => match key.as_str() {
            "base_dir" => println!("{}", cfg.base_dir.display()),
            "device_id" => println!("{}", cfg.device_id),
            "archive_backend" => println!("{}", cfg.archive_backend),
            "browser_cmd" => println!("{}", cfg.browser_cmd),
            "auth_token" => println!("{}", cfg.auth_token),
            _ => return Err(anyhow::anyhow!("unknown key {key:?}")),
        },
        ConfigCmd::Set { key, value } => {
            match key.as_str() {
                "base_dir" => cfg.base_dir = value.into(),
                "device_id" => cfg.device_id = value,
                "archive_backend" => match value.as_str() {
                    "builtin" | "monolith" | "single-file" => cfg.archive_backend = value,
                    _ => {
                        return Err(anyhow::anyhow!(
                            "invalid archive_backend (expected builtin, monolith, or single-file)"
                        ))
                    }
                },
                "browser_cmd" => cfg.browser_cmd = value,
                "auth_token" => cfg.auth_token = value,
                _ => return Err(anyhow::anyhow!("unknown key {key:?}")),
            }
            liber_core::config::save_config_to(&path, &cfg)?;
            println!("Set {key} in {}", path.display());
        }
        ConfigCmd::List => println!("{}", serde_json::to_string_pretty(&cfg).unwrap_or_default()),
    }
    Ok(())
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    match cli.cmd {
        None => {
            if cli.list {
                run_list(ListArgs {
                    query: None,
                    sort: None,
                })
            } else if cli.serve {
                println!("serve: not implemented");
                Ok(())
            } else {
                println!("liber: no command given, see --help");
                Ok(())
            }
        }
        Some(cmd) => match cmd {
            Cmd::Add(a) => run_add(a),
            Cmd::List(a) => run_list(a),
            Cmd::Open(a) => run_open(&a.spec),
            Cmd::Config(a) => run_config(a.cmd),
            Cmd::Serve(a) => {
                println!("serve {}: not implemented", a.addr);
                Ok(())
            }
            _ => {
                println!("not implemented");
                Ok(())
            }
        },
    }
}
