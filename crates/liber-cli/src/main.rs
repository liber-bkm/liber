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
    #[arg(short, long)]
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

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    match cli.cmd {
        None => {
            if cli.list {
                println!("list: not implemented");
            } else if cli.serve {
                println!("serve: not implemented");
            } else {
                println!("liber: no command given, see --help");
            }
        }
        Some(cmd) => match cmd {
            Cmd::Serve(a) => println!("serve {}: not implemented", a.addr),
            _ => println!("not implemented"),
        },
    }
    Ok(())
}
