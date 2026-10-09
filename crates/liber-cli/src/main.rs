use clap::{Parser, Subcommand};

mod pick_tui;

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

    #[arg(long, global = true)]
    uuid: bool,
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
    Backup(BackupArgs),
    Restore(RestoreArgs),
    Init(InitArgs),
    Tags(TagsArgs),
    Folders(FoldersArgs),
    Auto(AutoArgs),
    History(HistoryArgs),
    Check(CheckArgs),
    Archive(ArchiveArgs),
    Attachments(AttachmentsArgs),
    Sync(SyncArgs),
    Config(ConfigArgs),
    Pick(PickArgs),
    Profile(ProfileArgs),
    Serve(ServeArgs),
    Discover(DiscoverArgs),
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
    #[arg(long)]
    attach: Vec<std::path::PathBuf>,
    #[arg(short, long)]
    interactive: bool,
}

#[derive(clap::Args)]
struct ListArgs {
    query: Option<String>,
    #[arg(long)]
    sort: Option<String>,
    #[arg(long)]
    deep: bool,
    #[arg(long = "in", value_name = "FIELDS")]
    r#in: Option<String>,
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
    #[arg(long)]
    title: Option<String>,
    #[arg(long)]
    description: Option<String>,
    #[arg(short, long, num_args(0..))]
    tags: Option<Vec<String>>,
    #[arg(short, long)]
    folder: Option<String>,
    #[arg(long)]
    markdown: bool,
    #[arg(long)]
    attach: Vec<std::path::PathBuf>,
    #[arg(long)]
    detach: Vec<String>,
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
    prune: bool,
    #[arg(long, requires = "prune")]
    compact_ids: bool,
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
    markdown: bool,
    #[arg(long)]
    archive: bool,
}

#[derive(clap::Args)]
struct TagsArgs {
    #[command(subcommand)]
    cmd: Option<TagsCmd>,
}

#[derive(clap::Args)]
struct BackupArgs {
    file: String,
}

#[derive(clap::Args)]
struct RestoreArgs {
    file: String,
    #[arg(long)]
    force: bool,
    #[arg(long)]
    yes: bool,
}

#[derive(clap::Args)]
struct InitArgs {}

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
    Add {
        #[arg(long)]
        match_: String,
        #[arg(long)]
        folder: Option<String>,
        #[arg(long)]
        tag: Vec<String>,
    },
    List,
    Edit {
        id: String,
        #[arg(long)]
        match_: Option<String>,
        #[arg(long)]
        folder: Option<String>,
        #[arg(long)]
        tag: Vec<String>,
        #[arg(long)]
        reapply: bool,
    },
    Apply {
        id: Option<String>,
    },
    Learn {
        #[arg(long, default_value_t = 3)]
        min: usize,
        #[arg(long)]
        create: bool,
    },
    Delete {
        id: String,
    },
}

#[derive(clap::Args)]
struct HistoryArgs {}

#[derive(clap::Args)]
struct CheckArgs {
    spec: Option<String>,
    #[arg(long)]
    apply: bool,
    #[arg(long, default_value_t = 12)]
    workers: usize,
    #[arg(
        long,
        value_name = "DURATION",
        conflicts_with = "stale_hours",
        help = "Skip bookmarks checked within DURATION, e.g. 720h or 7d"
    )]
    stale: Option<String>,
    #[arg(
        long,
        value_name = "HOURS",
        help = "Deprecated alias for --stale, in hours; use --stale instead"
    )]
    stale_hours: Option<u64>,
}

#[derive(clap::Args)]
struct ArchiveArgs {
    spec: String,
    #[arg(long)]
    backend: Option<String>,
}

#[derive(clap::Args)]
struct AttachmentsArgs {
    spec: String,
}

#[derive(clap::Args)]
struct SyncArgs {
    #[command(subcommand)]
    cmd: SyncCmd,
}

#[derive(Subcommand)]
enum SyncCmd {
    Export {
        file: String,
        #[arg(long)]
        since: Option<i64>,
    },
    Import {
        file: String,
    },
    Commit {
        #[arg(long, default_value = "liber sync")]
        message: String,
        #[arg(long)]
        push: bool,
    },
    Prune {
        #[arg(long, default_value_t = 90)]
        days: i64,
    },
    Push {
        url: String,
        #[arg(long)]
        token: Option<String>,
        #[arg(long)]
        since: Option<i64>,
    },
    Pull {
        url: String,
        #[arg(long)]
        token: Option<String>,
        #[arg(long)]
        since: Option<i64>,
    },
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
    Delete { name: String },
}

#[derive(clap::Args)]
struct ServeArgs {
    #[arg(long, default_value = "127.0.0.1:8080")]
    addr: String,
    #[arg(long)]
    auth_token: Option<String>,
    #[arg(long)]
    static_dir: Option<std::path::PathBuf>,
    #[arg(long)]
    qr: bool,
    #[arg(long)]
    no_mdns: bool,
}

#[derive(clap::Args)]
struct DiscoverArgs {
    #[arg(long, default_value_t = 5)]
    timeout: u64,
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

fn show_id(b: &liber_core::model::Bookmark, full: bool) -> String {
    if full {
        return b.uuid.to_string();
    }
    match b.short_id {
        Some(n) => n.to_string(),
        None => b.uuid.to_string()[..8].to_string(),
    }
}

pub(crate) fn artifact_tags(b: &liber_core::model::Bookmark) -> String {
    let mut s = String::new();
    if b.markdown_file.is_some() {
        s.push_str(" [md]");
    }
    if b.archive_file.is_some() {
        s.push_str(" [arch]");
    }
    s
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

fn prompt_line(label: &str) -> String {
    use std::io::{self, Write};
    print!("{label}: ");
    let _ = io::stdout().flush();
    let mut line = String::new();
    if io::stdin().read_line(&mut line).is_err() {
        return String::new();
    }
    line.trim().to_string()
}

fn prompt_default(label: &str, def: &str) -> String {
    use std::io::{self, Write};
    if def.trim().is_empty() {
        return prompt_line(label);
    }
    print!("{label} [{def}]: ");
    let _ = io::stdout().flush();
    let mut line = String::new();
    if io::stdin().read_line(&mut line).is_err() {
        return def.to_string();
    }
    let line = line.trim().to_string();
    if line.is_empty() {
        def.to_string()
    } else {
        line
    }
}

fn split_tags_line(line: &str) -> Vec<String> {
    line.split_whitespace().map(str::to_string).collect()
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

fn run_add(a: AddArgs, full: bool) -> anyhow::Result<()> {
    use std::io::IsTerminal;
    let (_, mut store) = load_store()?;
    let interactive = if a.interactive {
        if !std::io::stdin().is_terminal() {
            return Err(anyhow::anyhow!("add --interactive needs a terminal"));
        }
        true
    } else {
        false
    };
    if interactive {
        if let Some(dup) = store.find_by_url(&liber_core::slug::normalize_url(&a.url))? {
            println!(
                "Already bookmarked: [{}] {} (folder: {})",
                show_id(&dup, full),
                dup.title,
                display_folder(&dup.folder)
            );
            if !confirm("Add it anyway?") {
                println!("Cancelled.");
                return Ok(());
            }
        }
    }
    let fetched_title = match a.title {
        Some(t) if !t.trim().is_empty() => Some(t),
        _ => {
            println!("Fetching title for {} ...", a.url);
            let fetched = liber_core::create::fetch_title(&liber_core::slug::normalize_url(&a.url));
            if fetched.trim().is_empty() {
                None
            } else {
                Some(fetched)
            }
        }
    };
    let (title, description, tags, folder) = if interactive {
        let title = Some(prompt_default(
            "Title",
            fetched_title.as_deref().unwrap_or(""),
        ));
        let description = prompt_default("Description", a.description.as_deref().unwrap_or(""));
        let tags = split_tags_line(&prompt_default("Tags (space separated)", &a.tags.join(" ")));
        let folder = prompt_default("Folder", a.folder.as_deref().unwrap_or(""));
        (title, description, tags, folder)
    } else {
        (
            fetched_title,
            a.description.unwrap_or_default(),
            a.tags,
            a.folder.unwrap_or_default(),
        )
    };
    let markdown = if interactive {
        a.markdown || confirm("Add markdown notes?")
    } else {
        a.markdown
    };
    let opts = liber_core::create::CreateOptions {
        title,
        description,
        tags,
        folder,
        markdown,
    };
    match liber_core::create::create_bookmark(&mut store, &a.url, opts) {
        Err(liber_core::CoreError::Duplicate(_)) => {
            let dup = store
                .find_by_url(&liber_core::slug::normalize_url(&a.url))?
                .unwrap();
            println!(
                "Already bookmarked: [{}] {} (folder: {})",
                show_id(&dup, full),
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
            println!("\nSaved [{}] {}", show_id(&b, full), b.title);
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
            if a.archive {
                match liber_core::archive::archive_bookmark(&mut store, &b.uuid, None) {
                    Ok(warnings) => {
                        for w in warnings {
                            println!("warning: {w}");
                        }
                        println!("  archive: attached");
                    }
                    Err(e) => println!("warning: archive failed: {e}"),
                }
            }
            for path in &a.attach {
                match liber_core::attach::attach_file(&mut store, &b.uuid, path) {
                    Ok(at) => println!("  attach: {}", at.name),
                    Err(e) => println!("warning: could not attach {}: {e}", path.display()),
                }
            }
            if interactive {
                run_add_attachments(&mut store, &b.uuid)?;
            }
            Ok(())
        }
    }
}

fn run_add_attachments(
    store: &mut liber_core::store::Store,
    uuid: &uuid::Uuid,
) -> anyhow::Result<()> {
    loop {
        let current = store.get(uuid)?.map(|b| b.attachments).unwrap_or_default();
        for (i, at) in current.iter().enumerate() {
            println!("  {}. {}", i + 1, at.name);
        }
        let line = prompt_line("Attachments (path to attach, rm <num|name>, empty to finish)");
        let line = line.trim().to_string();
        if line.is_empty() {
            return Ok(());
        }
        if let Some(which) = line
            .strip_prefix("rm ")
            .map(str::trim)
            .filter(|s| !s.is_empty())
        {
            match liber_core::attach::detach_attachment(store, uuid, which) {
                Ok(name) => println!("Detached {name}"),
                Err(e) => println!("warning: {e}"),
            }
            continue;
        }
        let path = std::path::PathBuf::from(&line);
        match liber_core::attach::attach_file(store, uuid, &path) {
            Ok(at) => println!("  attach: {}", at.name),
            Err(e) => println!("warning: could not attach {}: {e}", path.display()),
        }
    }
}

fn run_list(a: ListArgs, full: bool) -> anyhow::Result<()> {
    let (_, store) = load_store()?;
    let sort = liber_core::search::resolve_sort_mode(a.sort.as_deref(), a.query.as_deref())?;
    let scope = match &a.r#in {
        Some(s) => liber_core::search::parse_field_list(s)?,
        None => liber_core::search::SearchFields::all(),
    };
    let filter = liber_core::store::BookmarkFilter {
        query: a.query.clone(),
        scope,
        ..Default::default()
    };
    let (mut found, _) = store.query_bookmarks(&filter, sort, usize::MAX / 2, 0)?;
    if a.deep {
        if let Some(q) = &a.query {
            if !q.trim().is_empty() {
                let mut seen: std::collections::HashSet<uuid::Uuid> =
                    found.iter().map(|b| b.uuid).collect();
                for uuid in liber_core::search::deep_search_uuids(&store, q, 200, &scope)? {
                    if seen.insert(uuid) {
                        if let Some(b) = store.get(&uuid)? {
                            found.push(b);
                        }
                    }
                }
            }
        }
    }
    let query = a.query.unwrap_or_default();
    let ordered = liber_core::search::order_results(found, &query, sort);
    for b in ordered {
        println!(
            "[{}] {} ({}) {}{}",
            show_id(&b, full),
            b.title,
            display_folder(&b.folder),
            b.url,
            artifact_tags(&b)
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
                .filter(|b| liber_core::search::bookmark_matches_query(b, spec, &fields))
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

fn run_edit(a: EditArgs, full: bool) -> anyhow::Result<()> {
    use std::io::IsTerminal;
    let (_, mut store) = load_store()?;
    let tokens = liber_core::idspec::parse_id_spec(&a.spec)?;
    let targets = store.resolve_spec(&tokens)?;
    if targets.is_empty() {
        return Err(anyhow::anyhow!("no bookmarks matching {:?}", a.spec));
    }
    let has_flags = a.url.is_some()
        || a.title.is_some()
        || a.description.is_some()
        || a.tags.is_some()
        || a.folder.is_some()
        || a.markdown
        || !a.attach.is_empty()
        || !a.detach.is_empty();
    if !has_flags {
        if !std::io::stdin().is_terminal() {
            return Err(anyhow::anyhow!(
                "edit needs flags or a terminal (see `liber edit --help`)"
            ));
        }
        let chosen = if targets.len() == 1 {
            targets[0].clone()
        } else {
            match pick_tui::run_tui(targets, "")? {
                Some(b) => b,
                None => return Err(anyhow::anyhow!("no bookmark picked")),
            }
        };
        return run_edit_tui_flow(&mut store, &chosen.uuid);
    }
    for b in targets {
        let opts = liber_core::edit::EditOptions {
            title: a.title.clone(),
            description: a.description.clone(),
            tags: a.tags.clone(),
            folder: a.folder.clone(),
            url: a.url.clone(),
            add_markdown: a.markdown,
        };
        let out = liber_core::edit::edit_bookmark(&mut store, &b.uuid, opts)?;
        for path in &a.attach {
            match liber_core::attach::attach_file(&mut store, &out.uuid, path) {
                Ok(at) => println!("Attached {}", at.name),
                Err(e) => println!("warning: could not attach {}: {e}", path.display()),
            }
        }
        for which in &a.detach {
            match liber_core::attach::detach_attachment(&mut store, &out.uuid, which) {
                Ok(name) => println!("Detached {name}"),
                Err(e) => println!("warning: {e}"),
            }
        }
        println!("Updated [{}] {}", show_id(&out, full), out.title);
    }
    Ok(())
}

fn run_delete(spec: &str, yes: bool, full: bool) -> anyhow::Result<()> {
    let (_, mut store) = load_store()?;
    let tokens = liber_core::idspec::parse_id_spec(spec)?;
    let targets = store.resolve_spec(&tokens)?;
    if targets.is_empty() {
        return Err(anyhow::anyhow!("no bookmarks matching {spec:?}"));
    }
    if targets.len() == 1 {
        let b = &targets[0];
        if !yes && !confirm(&format!("Delete [{}] {}?", show_id(b, full), b.title)) {
            println!("Cancelled.");
            return Ok(());
        }
    } else {
        println!("About to delete {} bookmarks:", targets.len());
        for b in &targets {
            println!("  [{}] {}", show_id(b, full), b.title);
        }
        if !yes && !confirm(&format!("Delete all {}?", targets.len())) {
            println!("Cancelled.");
            return Ok(());
        }
    }
    for b in &targets {
        liber_core::edit::delete_bookmark_with_files(&mut store, &b.uuid)?;
    }
    if targets.len() == 1 {
        println!(
            "Deleted [{}] {}",
            show_id(&targets[0], full),
            targets[0].title
        );
    } else {
        println!("Deleted {} bookmark(s).", targets.len());
    }
    Ok(())
}

fn run_serve(
    addr: &str,
    token_flag: Option<&str>,
    static_dir: Option<std::path::PathBuf>,
    qr: bool,
    no_mdns: bool,
) -> anyhow::Result<()> {
    let (cfg, _) = liber_core::config::load_config()?;
    let token = cfg.resolve_auth_token(token_flag.unwrap_or(""));
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    rt.block_on(liber_server::serve(
        cfg,
        token,
        addr,
        static_dir,
        liber_server::ServeOptions { qr, mdns: !no_mdns },
    ))
}

fn run_discover(timeout_secs: u64) -> anyhow::Result<()> {
    let peers = liber_core::discovery::browse(std::time::Duration::from_secs(timeout_secs.max(1)))?;
    if peers.is_empty() {
        println!("No liber servers found on the local network.");
        return Ok(());
    }
    for p in &peers {
        let device = if p.device_id.trim().is_empty() {
            "(unknown device)".to_string()
        } else {
            format!("(device {})", p.device_id)
        };
        println!("{} {} {device}", p.instance, p.url());
    }
    Ok(())
}

fn run_tags(cmd: Option<TagsCmd>) -> anyhow::Result<()> {
    let (_, mut store) = load_store()?;
    match cmd {
        None | Some(TagsCmd::List) => {
            let counts = liber_core::taxonomy::tag_counts(&store)?;
            if counts.is_empty() {
                println!("No tags yet.");
            }
            for (name, n) in counts {
                println!("{name:<30} {n}");
            }
        }
        Some(TagsCmd::Rename { from, to }) => {
            let changed = liber_core::taxonomy::rename_tag(&mut store, &from, &to)?;
            if changed.is_empty() {
                println!("No bookmarks have the tag {from:?}.");
            } else {
                println!(
                    "Renamed tag {from:?} to {to:?} on {} bookmark(s).",
                    changed.len()
                );
            }
        }
        Some(TagsCmd::Delete { name }) => {
            let changed = liber_core::taxonomy::delete_tag(&mut store, &name)?;
            if changed.is_empty() {
                println!("No bookmarks have the tag {name:?}.");
            } else {
                println!("Removed tag {name:?} from {} bookmark(s).", changed.len());
            }
        }
    }
    Ok(())
}

fn run_folders(cmd: Option<FoldersCmd>) -> anyhow::Result<()> {
    let (_, mut store) = load_store()?;
    match cmd {
        None | Some(FoldersCmd::List) => {
            let counts = liber_core::taxonomy::folder_counts(&store)?;
            if counts.is_empty() {
                println!("No folders yet, everything is at the root.");
            }
            for (name, n) in counts {
                println!("{name:<30} {n}");
            }
        }
        Some(FoldersCmd::Rename { from, to }) => {
            let changed = liber_core::taxonomy::rename_folder(&mut store, &from, &to)?;
            if changed.is_empty() {
                println!("No bookmarks are in folder {from:?}.");
            } else {
                println!(
                    "Moved {} bookmark(s) from {from:?} to {to:?}.",
                    changed.len()
                );
            }
        }
        Some(FoldersCmd::Delete { name }) => {
            let changed = liber_core::taxonomy::delete_folder(&mut store, &name)?;
            if changed.is_empty() {
                println!("No bookmarks are in folder {name:?}.");
            } else {
                println!("Moved {} bookmark(s) to the root.", changed.len());
            }
        }
    }
    Ok(())
}

fn run_auto(cmd: AutoCmd) -> anyhow::Result<()> {
    let (_, mut store) = load_store()?;
    match cmd {
        AutoCmd::Add {
            match_,
            folder,
            tag,
        } => {
            let (rule, changed) =
                liber_core::automation::create_rule(&mut store, match_, tag, folder)?;
            println!(
                "Added automation {}",
                liber_core::automation::describe_rule(&rule)
            );
            if !changed.is_empty() {
                println!("Applied to {} existing bookmark(s).", changed.len());
            }
        }
        AutoCmd::List => {
            let rules = store.list_rules()?;
            if rules.is_empty() {
                println!("No automations yet. Add one with: liber auto add --match <substring> --folder <folder>");
                return Ok(());
            }
            let counts = applied_counts(&store)?;
            for r in rules {
                println!(
                    "{} (applied to {} bookmark(s))",
                    liber_core::automation::describe_rule(&r),
                    counts.get(&r.id).copied().unwrap_or(0)
                );
            }
        }
        AutoCmd::Edit {
            id,
            match_,
            folder,
            tag,
            reapply,
        } => {
            let tags = if tag.is_empty() { None } else { Some(tag) };
            let (rule, changed) =
                liber_core::automation::edit_rule(&mut store, &id, match_, tags, folder, reapply)?;
            println!(
                "Updated automation {}",
                liber_core::automation::describe_rule(&rule)
            );
            if reapply {
                println!("Reapplied to {} bookmark(s).", changed.len());
            }
        }
        AutoCmd::Apply { id } => {
            let changed = liber_core::automation::apply_rules(&mut store, id.as_deref())?;
            println!("Applied automations to {} bookmark(s).", changed.len());
        }
        AutoCmd::Learn { min, create } => {
            let suggestions = liber_core::automation::suggest_rules(&store, min.max(2))?;
            if suggestions.is_empty() {
                println!("No rule suggestions: no host appears in one folder often enough.");
                return Ok(());
            }
            for s in suggestions {
                println!(
                    "{} bookmark(s) with host {} are in folder {:?}: liber auto add --match host:{} --folder {}",
                    s.count, s.host, s.folder, s.host, s.folder
                );
                if !create && !confirm("Create this rule?") {
                    continue;
                }
                let (rule, changed) = liber_core::automation::create_rule(
                    &mut store,
                    format!("host:{}", s.host),
                    vec![],
                    Some(s.folder.clone()),
                )?;
                println!(
                    "Added automation {} (applied to {} existing bookmark(s)).",
                    liber_core::automation::describe_rule(&rule),
                    changed.len()
                );
            }
        }
        AutoCmd::Delete { id } => {
            if store.delete_rule(&id)? {
                println!(
                    "Deleted automation [{id}]. Bookmarks it already classified are left as-is."
                );
            } else {
                return Err(anyhow::anyhow!("no automation with id {id:?}"));
            }
        }
    }
    Ok(())
}

fn applied_counts(
    store: &liber_core::store::Store,
) -> anyhow::Result<std::collections::HashMap<String, usize>> {
    let mut counts = std::collections::HashMap::new();
    for b in store.list()? {
        for a in &b.applied_rules {
            *counts.entry(a.rule_id.clone()).or_insert(0) += 1;
        }
    }
    Ok(counts)
}

fn run_check(a: CheckArgs, full: bool) -> anyhow::Result<()> {
    use liber_core::check::{parse_stale_duration, quarantine_bookmark, CheckStatus};
    use std::time::Duration;

    let (_, mut store) = load_store()?;
    let tokens = match &a.spec {
        Some(s) if !s.trim().is_empty() => Some(liber_core::idspec::parse_id_spec(s)?),
        _ => None,
    };
    let stale = match (&a.stale, a.stale_hours) {
        (Some(s), _) => Some(parse_stale_duration(s)?),
        (None, Some(h)) => {
            eprintln!("warning: --stale-hours is deprecated, use --stale (e.g. --stale {h}h)");
            Some(Duration::from_secs(h.saturating_mul(3600)))
        }
        (None, None) => None,
    };
    let (targets, fresh) =
        liber_core::check::resolve_check_targets(&store, tokens.as_deref(), stale)?;
    if fresh > 0 {
        println!("Skipped {fresh} freshly checked bookmark(s).");
    }
    if targets.is_empty() {
        println!("No bookmarks to check.");
        return Ok(());
    }
    let client = liber_core::check::check_client()?;
    let outcomes = liber_core::check::scan_targets(&client, targets, a.workers, |done, total| {
        eprintln!("\rChecking {done}/{total}...");
    });
    eprintln!();
    let mut moved = vec![];
    let mut dead = vec![];
    let mut uncertain = vec![];
    for o in &outcomes {
        match o.result.status {
            CheckStatus::Moved => moved.push(o),
            CheckStatus::Dead => dead.push(o),
            CheckStatus::Uncertain => uncertain.push(o),
            CheckStatus::Ok => {}
        }
    }
    for o in &outcomes {
        store.stamp_check(&o.bookmark.uuid, Some(o.result.status.as_str()))?;
    }
    let ok = outcomes.len() - moved.len() - dead.len() - uncertain.len();
    println!(
        "{} ok, {} moved, {} dead, {} uncertain (of {} checked)",
        ok,
        moved.len(),
        dead.len(),
        uncertain.len(),
        outcomes.len()
    );
    for o in &moved {
        println!(
            "[{}] {}\n    moved -> {} ({})",
            show_id(&o.bookmark, full),
            o.bookmark.title,
            o.result.target.as_deref().unwrap_or("?"),
            o.result.detail
        );
    }
    for o in &dead {
        println!(
            "[{}] {} dead ({})",
            show_id(&o.bookmark, full),
            o.bookmark.title,
            o.result.detail
        );
    }
    for o in &uncertain {
        println!(
            "[{}] {} uncertain ({})",
            show_id(&o.bookmark, full),
            o.bookmark.title,
            o.result.detail
        );
    }
    if !a.apply {
        return Ok(());
    }
    if std::io::IsTerminal::is_terminal(&std::io::stdin()) {
        return run_check_interactive(&mut store, &moved, &dead, &uncertain, full);
    }
    let (mut updated, mut quarantined, mut skipped) = (0, 0, 0);
    for o in &moved {
        let target = o.result.target.clone().unwrap_or_default();
        match liber_core::edit::edit_bookmark(
            &mut store,
            &o.bookmark.uuid,
            liber_core::edit::EditOptions {
                url: Some(target.clone()),
                ..Default::default()
            },
        ) {
            Ok(_) => {
                updated += 1;
                println!("Updated [{}].", show_id(&o.bookmark, full));
            }
            Err(liber_core::CoreError::Duplicate(_)) => {
                skipped += 1;
                println!(
                    "Skipped [{}]: target URL already bookmarked.",
                    show_id(&o.bookmark, full)
                );
            }
            Err(e) => return Err(e.into()),
        }
    }
    for o in dead.iter().chain(uncertain.iter()) {
        if quarantine_bookmark(&mut store, &o.bookmark.uuid)? {
            quarantined += 1;
            println!("Quarantined [{}].", show_id(&o.bookmark, full));
        }
    }
    println!("Done: {updated} updated, {quarantined} quarantined, {skipped} skipped.");
    Ok(())
}

enum CheckChoice {
    Delete,
    Quarantine,
    Skip,
}

fn prompt_check_action(prompt: &str) -> CheckChoice {
    use std::io::{self, Write};
    print!("{prompt} [y] delete / [q] quarantine / [N] skip ");
    let _ = io::stdout().flush();
    let mut line = String::new();
    if io::stdin().read_line(&mut line).is_err() {
        return CheckChoice::Skip;
    }
    match line.trim().to_lowercase().as_str() {
        "y" | "yes" | "delete" => CheckChoice::Delete,
        "q" | "quarantine" => CheckChoice::Quarantine,
        _ => CheckChoice::Skip,
    }
}

fn confirm_yes(prompt: &str) -> bool {
    use std::io::{self, Write};
    print!("{prompt} [Y/n] ");
    let _ = io::stdout().flush();
    let mut line = String::new();
    if io::stdin().read_line(&mut line).is_err() {
        return false;
    }
    !matches!(line.trim().to_lowercase().as_str(), "n" | "no")
}

fn run_check_interactive(
    store: &mut liber_core::store::Store,
    moved: &[&liber_core::check::CheckOutcome],
    dead: &[&liber_core::check::CheckOutcome],
    uncertain: &[&liber_core::check::CheckOutcome],
    full: bool,
) -> anyhow::Result<()> {
    use liber_core::check::{quarantine_bookmark, refetch_title};
    let (mut updated, mut retitled, mut deleted, mut quarantined, mut skipped) = (0, 0, 0, 0, 0);
    for o in moved {
        let target = o.result.target.clone().unwrap_or_default();
        if !confirm_yes(&format!(
            "Update [{}] URL to {target}?",
            show_id(&o.bookmark, full)
        )) {
            skipped += 1;
            continue;
        }
        match liber_core::edit::edit_bookmark(
            store,
            &o.bookmark.uuid,
            liber_core::edit::EditOptions {
                url: Some(target.clone()),
                ..Default::default()
            },
        ) {
            Ok(_) => {
                updated += 1;
                println!("Updated [{}].", show_id(&o.bookmark, full));
            }
            Err(liber_core::CoreError::Duplicate(_)) => {
                skipped += 1;
                println!(
                    "Skipped [{}]: target URL already bookmarked.",
                    show_id(&o.bookmark, full)
                );
                continue;
            }
            Err(e) => return Err(e.into()),
        }
        let title = refetch_title(&target);
        if !title.is_empty()
            && title != o.bookmark.title
            && confirm_yes(&format!(
                "Update [{}] title to {title:?}?",
                show_id(&o.bookmark, full)
            ))
        {
            liber_core::edit::edit_bookmark(
                store,
                &o.bookmark.uuid,
                liber_core::edit::EditOptions {
                    title: Some(title),
                    ..Default::default()
                },
            )?;
            retitled += 1;
            println!("Retitled [{}].", show_id(&o.bookmark, full));
        }
    }
    for o in dead.iter().chain(uncertain.iter()) {
        match prompt_check_action(&format!(
            "Delete dead [{}] {}?",
            show_id(&o.bookmark, full),
            o.bookmark.title
        )) {
            CheckChoice::Delete => {
                liber_core::edit::delete_bookmark_with_files(store, &o.bookmark.uuid)?;
                deleted += 1;
                println!("Deleted [{}].", show_id(&o.bookmark, full));
            }
            CheckChoice::Quarantine => {
                if quarantine_bookmark(store, &o.bookmark.uuid)? {
                    quarantined += 1;
                    println!("Quarantined [{}].", show_id(&o.bookmark, full));
                }
            }
            CheckChoice::Skip => skipped += 1,
        }
    }
    println!(
        "Done: {updated} updated, {retitled} retitled, {deleted} deleted, {quarantined} quarantined, {skipped} skipped."
    );
    Ok(())
}

fn run_archive(spec: &str, backend: Option<&str>, full: bool) -> anyhow::Result<()> {
    let (_, mut store) = load_store()?;
    let tokens = liber_core::idspec::parse_id_spec(spec)?;
    let targets = store.resolve_spec(&tokens)?;
    if targets.is_empty() {
        return Err(anyhow::anyhow!("no bookmarks matching {spec:?}"));
    }
    for b in targets {
        match liber_core::archive::archive_bookmark(&mut store, &b.uuid, backend) {
            Ok(warnings) => {
                for w in warnings {
                    println!("warning: {w}");
                }
                println!("Archived [{}].", show_id(&b, full));
            }
            Err(e) => println!("warning: archive failed for [{}]: {e}", show_id(&b, full)),
        }
    }
    Ok(())
}

fn run_import(a: ImportArgs) -> anyhow::Result<()> {
    let (_, mut store) = load_store()?;
    let data = std::fs::read(&a.file)?;
    let content = String::from_utf8_lossy(&data);
    let report = liber_core::import::import_data(&mut store, &content, a.markdown, a.archive)?;
    for w in &report.warnings {
        println!("{w}");
    }
    if report.added == 0 && report.skipped_dup == 0 && report.skipped_bad == 0 {
        println!("No bookmarks found in that file, is it a browser bookmark export?");
        return Ok(());
    }
    println!("Imported {} bookmark(s).", report.added);
    if report.skipped_dup > 0 {
        println!("Skipped {} already in your collection.", report.skipped_dup);
    }
    if report.skipped_bad > 0 {
        println!("Skipped {} entries with no URL.", report.skipped_bad);
    }
    Ok(())
}

fn run_export(a: ExportArgs) -> anyhow::Result<()> {
    let (cfg, store) = load_store()?;
    if a.site {
        let out_dir = match &a.out {
            Some(d) => std::path::PathBuf::from(d),
            None => cfg.profile_dir().join("site"),
        };
        let index = liber_core::export::export_site(&store, &out_dir)?;
        let n = store.list()?.len();
        println!("Exported {n} bookmark(s) to {}", index.display());
        return Ok(());
    }
    if a.bookmarks {
        let Some(path) = &a.out else {
            return Err(anyhow::anyhow!("--bookmarks needs an output file"));
        };
        let doc = liber_core::export::write_netscape_export(&store)?;
        std::fs::write(path, doc)?;
        let n = store.list()?.len();
        println!("Exported {n} bookmark(s) to {path}");
        return Ok(());
    }
    Err(anyhow::anyhow!("specify --site or --bookmarks"))
}

fn sync_token(flag: &Option<String>) -> String {
    flag.clone()
        .filter(|t| !t.trim().is_empty())
        .unwrap_or_else(|| std::env::var("LIBER_AUTH_TOKEN").unwrap_or_default())
}

fn print_merge_report(total: usize, rep: &liber_core::sync::MergeReport) {
    println!(
        "Merged {total}: {} inserted, {} merged, {} deduped, {} deleted, {} rules, {} renumbered.",
        rep.inserted, rep.merged, rep.deduped, rep.deleted, rep.rules, rep.renumbered
    );
    if rep.short_ids_assigned > 0 {
        println!("Assigned {} short id(s).", rep.short_ids_assigned);
    }
    if rep.short_ids_compacted > 0 {
        println!(
            "Compacted {} bookmark(s) to dense short ids.",
            rep.short_ids_compacted
        );
    }
}

fn run_sync(cmd: SyncCmd) -> anyhow::Result<()> {
    use liber_core::sync::{
        export_bundle, git_snapshot, prune_oplog, read_bundle, replay_entries, write_bundle,
    };
    use std::path::PathBuf;
    let (cfg, mut store) = load_store()?;
    match cmd {
        SyncCmd::Export { file, since } => {
            let entries = export_bundle(&store, since)?;
            write_bundle(&PathBuf::from(&file), &entries)?;
            println!(
                "Exported {} oplog entr{} to {file}.",
                entries.len(),
                if entries.len() == 1 { "y" } else { "ies" }
            );
        }
        SyncCmd::Import { file } => {
            let entries = read_bundle(&PathBuf::from(&file))?;
            let rep = replay_entries(&mut store, &entries)?;
            print_merge_report(entries.len(), &rep);
        }
        SyncCmd::Push { url, token, since } => {
            let entries = export_bundle(&store, since)?;
            let rep = liber_core::sync_http::push_bundle(&url, &sync_token(&token), &entries)?;
            print_merge_report(entries.len(), &rep);
        }
        SyncCmd::Pull { url, token, since } => {
            let entries = liber_core::sync_http::pull_bundle(&url, &sync_token(&token), since)?;
            let rep = replay_entries(&mut store, &entries)?;
            print_merge_report(entries.len(), &rep);
        }
        SyncCmd::Commit { message, push } => {
            if git_snapshot(&cfg.profile_dir(), &message, push)? {
                println!("Committed sync snapshot.");
            } else {
                println!("Not a git repository, nothing committed (repos are never initialized automatically).");
            }
        }
        SyncCmd::Prune { days } => {
            let n = prune_oplog(&mut store, days)?;
            println!("Pruned {n} oplog entries older than {days} days.");
        }
    }
    Ok(())
}

fn run_backup(file: &str) -> anyhow::Result<()> {
    let (cfg, _) = load_store()?;
    let dir = cfg.profile_dir();
    let out = std::path::PathBuf::from(file);
    liber_core::backup::create_backup(&dir, &out, &cfg.effective_device_id())?;
    println!("Backed up {} to {}.", dir.display(), out.display());
    Ok(())
}

fn run_restore(file: &str, force: bool, yes: bool) -> anyhow::Result<()> {
    let (cfg, _) = load_store()?;
    let dest = cfg.profile_dir();
    let db = liber_core::backup::profile_db_path(&dest);
    if db.exists() && !force {
        return Err(anyhow::anyhow!(
            "library already exists in {} (use --force to replace it)",
            dest.display()
        ));
    }
    if db.exists()
        && !yes
        && !confirm(&format!(
            "Replace library in {} with backup?",
            dest.display()
        ))
    {
        println!("Cancelled.");
        return Ok(());
    }
    let manifest = liber_core::backup::restore_backup(&std::path::PathBuf::from(file), &dest)?;
    println!(
        "Restored backup from {} ({}) to {}.",
        manifest.device_id,
        manifest.created_at,
        dest.display()
    );
    run_reindex(false, false)
}

fn run_init() -> anyhow::Result<()> {
    use std::io::IsTerminal;
    if !std::io::stdin().is_terminal() {
        return Err(anyhow::anyhow!("init needs a terminal"));
    }
    let path = liber_core::config::config_path()?;
    if path.exists() {
        return Err(anyhow::anyhow!(
            "config already exists at {} (use config set to change it)",
            path.display()
        ));
    }
    let mut cfg = liber_core::config::default_config();
    let base = prompt_default("Base directory", &cfg.base_dir.display().to_string());
    if base.trim().is_empty() {
        return Err(anyhow::anyhow!("base directory can't be empty"));
    }
    cfg.base_dir = base.trim().into();
    let device = prompt_default("Device id", &uuid::Uuid::new_v4().to_string());
    cfg.device_id = if device.trim().is_empty() {
        uuid::Uuid::new_v4().to_string()
    } else {
        device
    };
    cfg.archive_backend = loop {
        let b = prompt_default(
            "Archive backend (auto, builtin, browser, single-file, monolith)",
            "builtin",
        );
        match liber_core::archive::parse_backend(&b) {
            Ok(_) => break b,
            Err(e) => println!("warning: {e}"),
        }
    };
    cfg.auth_token = prompt_line("Auth token (empty means open)");
    liber_core::config::save_config_to(&path, &cfg)?;
    let _ = liber_core::store::Store::open(cfg.clone())?;
    println!("Wrote config to {}.", path.display());
    println!("Library at {}.", cfg.profile_dir().display());
    if let Ok(base) = std::env::var("LIBER_BASE_DIR") {
        if !base.trim().is_empty() {
            println!("Note: LIBER_BASE_DIR overrides the file value at runtime.");
        }
    }
    println!("Next: `liber add <url>` or `liber import <file>`.");
    Ok(())
}

fn run_reindex(prune: bool, compact_ids: bool) -> anyhow::Result<()> {
    let (_, mut store) = load_store()?;
    let rep = liber_core::reindex::reindex(
        &mut store,
        liber_core::reindex::ReindexFlags { prune, compact_ids },
    )?;
    println!(
        "Reindexed: {} adopted, {} markdown relinked, {} archives relinked, {} conflicts swept, {} attachments quarantined, {} indexed.",
        rep.adopted,
        rep.relinked_markdown,
        rep.relinked_archive,
        rep.swept_conflicts,
        rep.quarantined_attachments,
        rep.indexed
    );
    if !rep.pending.is_empty() {
        println!("Pending (missing files, kept):");
        for uuid in &rep.pending {
            println!("  {uuid}");
        }
        println!("Run with --prune to drop them (files move to unindexed/).");
    }
    if rep.pruned > 0 {
        println!("Pruned {} bookmark(s).", rep.pruned);
    }
    if rep.short_ids_compacted > 0 {
        println!(
            "Compacted {} bookmark(s) to dense short ids.",
            rep.short_ids_compacted
        );
    }
    Ok(())
}

fn run_attachments(spec: &str, full: bool) -> anyhow::Result<()> {
    let (_, store) = load_store()?;
    let tokens = liber_core::idspec::parse_id_spec(spec)?;
    let targets = store.resolve_spec(&tokens)?;
    if targets.is_empty() {
        return Err(anyhow::anyhow!("no bookmarks matching {spec:?}"));
    }
    for b in targets {
        println!("[{}] {}", show_id(&b, full), b.title);
        if b.attachments.is_empty() {
            println!("  (none)");
        }
        for (i, at) in b.attachments.iter().enumerate() {
            println!("  {}) {}", i + 1, at.name);
        }
    }
    Ok(())
}

fn run_history(full: bool) -> anyhow::Result<()> {
    use liber_core::search::SortMode;
    use liber_core::store::BookmarkFilter;
    let (_, store) = load_store()?;
    let filter = BookmarkFilter {
        opened_only: true,
        ..Default::default()
    };
    let (found, _) = store.query_bookmarks(&filter, SortMode::Visited, usize::MAX / 2, 0)?;
    if found.is_empty() {
        println!("Nothing opened yet.");
        return Ok(());
    }
    for b in found {
        println!(
            "[{}] {} ({}) {} ({}x){}",
            show_id(&b, full),
            b.title,
            display_folder(&b.folder),
            b.url,
            b.open_count,
            artifact_tags(&b)
        );
    }
    Ok(())
}

fn run_profile(cmd: Option<ProfileCmd>) -> anyhow::Result<()> {
    use liber_core::profile::{delete_profile, list_profiles, switch_profile};
    let (mut cfg, path) = liber_core::config::load_config()?;
    match cmd {
        None | Some(ProfileCmd::List) => {
            println!("Profiles:");
            for p in list_profiles(&cfg) {
                let mark = if p.active { "*" } else { " " };
                println!("  {mark} {}", p.name);
            }
        }
        Some(ProfileCmd::Switch { name }) => {
            let active = switch_profile(&mut cfg, &name)?;
            liber_core::config::save_config_to(&path, &cfg)?;
            if active == "default" {
                println!("Switched to the default profile.");
            } else {
                println!("Switched to profile {active:?}.");
            }
        }
        Some(ProfileCmd::Delete { name }) => {
            let deleted = delete_profile(&mut cfg, &name)?;
            liber_core::config::save_config_to(&path, &cfg)?;
            println!("Deleted profile {deleted:?} (untracked only, files left on disk).");
        }
    }
    Ok(())
}

fn run_completion(shell: &str) -> anyhow::Result<()> {
    use clap::CommandFactory;
    use clap_complete::{generate, shells};
    let mut cmd = Cli::command();
    match shell.to_lowercase().as_str() {
        "bash" => generate(shells::Bash, &mut cmd, "liber", &mut std::io::stdout()),
        "zsh" => generate(shells::Zsh, &mut cmd, "liber", &mut std::io::stdout()),
        "fish" => generate(shells::Fish, &mut cmd, "liber", &mut std::io::stdout()),
        "powershell" => generate(
            shells::PowerShell,
            &mut cmd,
            "liber",
            &mut std::io::stdout(),
        ),
        "elvish" => generate(shells::Elvish, &mut cmd, "liber", &mut std::io::stdout()),
        other => {
            return Err(anyhow::anyhow!(
                "unknown shell {other:?} (bash, zsh, fish, powershell, elvish)"
            ))
        }
    }
    Ok(())
}

fn run_edit_tui_flow(
    store: &mut liber_core::store::Store,
    uuid: &uuid::Uuid,
) -> anyhow::Result<()> {
    let Some(b) = store.get(uuid)? else {
        return Err(anyhow::anyhow!("bookmark gone"));
    };
    match pick_tui::run_edit_tui(&b)? {
        None => {
            println!("Cancelled.");
            Ok(())
        }
        Some(draft) => {
            let applied = liber_core::edit::apply_edit(store, uuid, draft)?;
            for w in &applied.warnings {
                println!("{w}");
            }
            let id = match applied.bookmark.short_id {
                Some(n) => n.to_string(),
                None => applied.bookmark.uuid.to_string(),
            };
            println!("Updated [{id}] {}", applied.bookmark.title);
            Ok(())
        }
    }
}

fn run_pick(query: Option<&str>) -> anyhow::Result<()> {
    use std::io::IsTerminal;
    let (_, store) = load_store()?;
    let targets = match query {
        Some(q) if !q.trim().is_empty() => liber_core::picker::search_targets(&store, q)?,
        _ => store.list()?,
    };
    if targets.is_empty() {
        return Err(anyhow::anyhow!(
            "no bookmarks matching {:?}",
            query.unwrap_or("")
        ));
    }
    if targets.len() == 1 {
        println!("{}", targets[0].url);
        return Ok(());
    }
    let shown: Vec<_> = targets.into_iter().take(100).collect();
    if !std::io::stdin().is_terminal() {
        return match pick_tui::confirm_numbered(&shown)? {
            Some(b) => {
                println!("{}", b.url);
                Ok(())
            }
            None => Err(anyhow::anyhow!("no bookmark picked")),
        };
    }
    match pick_tui::run_tui(shown, query.unwrap_or(""))? {
        Some(b) => {
            let (cfg, mut store) = load_store()?;
            match pick_tui::action_menu(&b)? {
                None => Err(anyhow::anyhow!("no bookmark picked")),
                Some(pick_tui::PickAction::Open) => {
                    store.record_open(&b.uuid)?;
                    open_in_browser(&cfg, &b.url)?;
                    Ok(())
                }
                Some(pick_tui::PickAction::OpenArchive) => {
                    let Some(rel) = &b.archive_file else {
                        return Err(anyhow::anyhow!("no archived copy for this bookmark"));
                    };
                    let path = store.cfg.archive_dir().join(rel);
                    let url = format!("file://{}", path.to_string_lossy());
                    store.record_open(&b.uuid)?;
                    open_in_browser(&cfg, &url)?;
                    Ok(())
                }
                Some(pick_tui::PickAction::OpenNotes) => {
                    match liber_core::edit::read_note_body(&store, &b.uuid)? {
                        Some(body) => println!("{body}"),
                        None => println!("No notes for this bookmark yet."),
                    }
                    Ok(())
                }
                Some(pick_tui::PickAction::Edit) => run_edit_tui_flow(&mut store, &b.uuid),
            }
        }
        None => Err(anyhow::anyhow!("no bookmark picked")),
    }
}

fn opt_dir(d: &Option<std::path::PathBuf>) -> String {
    d.as_ref()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|| "(default)".to_string())
}

fn run_config(cmd: ConfigCmd) -> anyhow::Result<()> {
    let (mut cfg, path) = liber_core::config::load_config()?;
    match cmd {
        ConfigCmd::Get { key } => match key.as_str() {
            "base_dir" => println!("{}", cfg.base_dir.display()),
            "html_dir" => println!("{}", opt_dir(&cfg.html_dir)),
            "markdown_dir" => println!("{}", opt_dir(&cfg.markdown_dir)),
            "archive_dir" => println!("{}", opt_dir(&cfg.archive_dir)),
            "attachment_dir" => println!("{}", opt_dir(&cfg.attachment_dir)),
            "device_id" => println!("{}", cfg.device_id),
            "archive_backend" => println!("{}", cfg.archive_backend),
            "browser_cmd" => println!("{}", cfg.browser_cmd),
            "browser_path" => println!("{}", cfg.browser_path),
            "editor_cmd" => println!("{}", cfg.editor_cmd),
            "singlefile_cmd" => println!("{}", cfg.singlefile_cmd),
            "singlefile_browser_path" => println!("{}", cfg.singlefile_browser_path),
            "monolith_cmd" => println!("{}", cfg.monolith_cmd),
            "auth_token" => println!("{}", cfg.auth_token),
            _ => return Err(anyhow::anyhow!("unknown key {key:?}")),
        },
        ConfigCmd::Set { key, value } => {
            if key == "auth_token" {
                cfg.auth_token = value;
            } else {
                liber_core::config::apply_setting(&mut cfg, &key, &value)
                    .map_err(|e| anyhow::anyhow!("{e}"))?;
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
                run_list(
                    ListArgs {
                        query: None,
                        sort: None,
                        deep: false,
                        r#in: None,
                    },
                    cli.uuid,
                )
            } else if cli.serve {
                println!("serve: not implemented");
                Ok(())
            } else {
                println!("liber: no command given, see --help");
                Ok(())
            }
        }
        Some(cmd) => match cmd {
            Cmd::Add(a) => run_add(a, cli.uuid),
            Cmd::List(a) => run_list(a, cli.uuid),
            Cmd::Open(a) => run_open(&a.spec),
            Cmd::Edit(a) => run_edit(a, cli.uuid),
            Cmd::Delete(a) => run_delete(&a.spec, a.yes, cli.uuid),
            Cmd::Tags(a) => run_tags(a.cmd),
            Cmd::Folders(a) => run_folders(a.cmd),
            Cmd::Auto(a) => run_auto(a.cmd),
            Cmd::Check(a) => run_check(a, cli.uuid),
            Cmd::Archive(a) => run_archive(&a.spec, a.backend.as_deref(), cli.uuid),
            Cmd::Attachments(a) => run_attachments(&a.spec, cli.uuid),
            Cmd::Import(a) => run_import(a),
            Cmd::Export(a) => run_export(a),
            Cmd::Backup(a) => run_backup(&a.file),
            Cmd::Restore(a) => run_restore(&a.file, a.force, a.yes),
            Cmd::Init(_) => run_init(),
            Cmd::Reindex(a) => run_reindex(a.prune, a.compact_ids),
            Cmd::Sync(a) => run_sync(a.cmd),
            Cmd::Config(a) => run_config(a.cmd),
            Cmd::Pick(a) => run_pick(a.query.as_deref()),
            Cmd::Serve(a) => run_serve(
                &a.addr,
                a.auth_token.as_deref(),
                a.static_dir.clone(),
                a.qr,
                a.no_mdns,
            ),
            Cmd::Discover(a) => run_discover(a.timeout),
            Cmd::History(_) => run_history(cli.uuid),
            Cmd::Profile(a) => run_profile(a.cmd),
            Cmd::Completion(a) => run_completion(&a.shell),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tags_split_on_whitespace() {
        assert_eq!(
            split_tags_line("rust  async\tweb"),
            vec!["rust", "async", "web"]
        );
        assert!(split_tags_line("   ").is_empty());
        assert!(split_tags_line("").is_empty());
    }

    #[test]
    fn add_interactive_flag_parses() {
        let cli = Cli::try_parse_from(["liber", "add", "-i", "https://example.com"]).unwrap();
        match cli.cmd {
            Some(Cmd::Add(a)) => {
                assert!(a.interactive);
                assert_eq!(a.url, "https://example.com");
            }
            _ => panic!("expected add subcommand"),
        }
        let cli = Cli::try_parse_from(["liber", "add", "https://example.com"]).unwrap();
        match cli.cmd {
            Some(Cmd::Add(a)) => assert!(!a.interactive),
            _ => panic!("expected add subcommand"),
        }
    }

    #[test]
    fn sync_push_pull_parse() {
        let cli = Cli::try_parse_from([
            "liber",
            "sync",
            "push",
            "http://192.168.1.10:8080",
            "--since",
            "5",
        ])
        .unwrap();
        match cli.cmd {
            Some(Cmd::Sync(a)) => match a.cmd {
                SyncCmd::Push { url, since, token } => {
                    assert_eq!(url, "http://192.168.1.10:8080");
                    assert_eq!(since, Some(5));
                    assert_eq!(token, None);
                }
                _ => panic!("expected sync push"),
            },
            _ => panic!("expected sync subcommand"),
        }
        let cli = Cli::try_parse_from(["liber", "sync", "pull", "http://x:8080", "--token", "t"])
            .unwrap();
        match cli.cmd {
            Some(Cmd::Sync(a)) => match a.cmd {
                SyncCmd::Pull { url, since, token } => {
                    assert_eq!(url, "http://x:8080");
                    assert_eq!(since, None);
                    assert_eq!(token, Some("t".to_string()));
                }
                _ => panic!("expected sync pull"),
            },
            _ => panic!("expected sync subcommand"),
        }
    }

    #[test]
    fn backup_restore_init_parse() {
        let cli = Cli::try_parse_from(["liber", "backup", "b.tar.gz"]).unwrap();
        match cli.cmd {
            Some(Cmd::Backup(a)) => assert_eq!(a.file, "b.tar.gz"),
            _ => panic!("expected backup subcommand"),
        }
        let cli = Cli::try_parse_from(["liber", "restore", "b.tar.gz", "--force"]).unwrap();
        match cli.cmd {
            Some(Cmd::Restore(a)) => {
                assert_eq!(a.file, "b.tar.gz");
                assert!(a.force);
                assert!(!a.yes);
            }
            _ => panic!("expected restore subcommand"),
        }
        let cli = Cli::try_parse_from(["liber", "init"]).unwrap();
        match cli.cmd {
            Some(Cmd::Init(_)) => {}
            _ => panic!("expected init subcommand"),
        }
    }

    #[test]
    fn serve_and_discover_parse() {
        let cli = Cli::try_parse_from([
            "liber",
            "serve",
            "--addr",
            "0.0.0.0:9000",
            "--qr",
            "--no-mdns",
        ])
        .unwrap();
        match cli.cmd {
            Some(Cmd::Serve(a)) => {
                assert_eq!(a.addr, "0.0.0.0:9000");
                assert!(a.qr);
                assert!(a.no_mdns);
            }
            _ => panic!("expected serve subcommand"),
        }
        let cli = Cli::try_parse_from(["liber", "discover", "--timeout", "2"]).unwrap();
        match cli.cmd {
            Some(Cmd::Discover(a)) => assert_eq!(a.timeout, 2),
            _ => panic!("expected discover subcommand"),
        }
    }
}
