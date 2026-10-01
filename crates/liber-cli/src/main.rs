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
    Archive(ArchiveArgs),
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
    markdown: bool,
    #[arg(long)]
    archive: bool,
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
    #[arg(long)]
    stale_hours: Option<u64>,
}

#[derive(clap::Args)]
struct ArchiveArgs {
    spec: String,
    #[arg(long)]
    backend: Option<String>,
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

fn run_edit(a: EditArgs) -> anyhow::Result<()> {
    let (_, mut store) = load_store()?;
    let tokens = liber_core::idspec::parse_id_spec(&a.spec)?;
    let targets = store.resolve_spec(&tokens)?;
    if targets.is_empty() {
        return Err(anyhow::anyhow!("no bookmarks matching {:?}", a.spec));
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
        println!("Updated [{}] {}", &out.uuid.to_string()[..8], out.title);
    }
    Ok(())
}

fn run_delete(spec: &str, yes: bool) -> anyhow::Result<()> {
    let (_, mut store) = load_store()?;
    let tokens = liber_core::idspec::parse_id_spec(spec)?;
    let targets = store.resolve_spec(&tokens)?;
    if targets.is_empty() {
        return Err(anyhow::anyhow!("no bookmarks matching {spec:?}"));
    }
    if targets.len() == 1 {
        let b = &targets[0];
        if !yes
            && !confirm(&format!(
                "Delete [{}] {}?",
                &b.uuid.to_string()[..8],
                b.title
            ))
        {
            println!("Cancelled.");
            return Ok(());
        }
    } else {
        println!("About to delete {} bookmarks:", targets.len());
        for b in &targets {
            println!("  [{}] {}", &b.uuid.to_string()[..8], b.title);
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
            &targets[0].uuid.to_string()[..8],
            targets[0].title
        );
    } else {
        println!("Deleted {} bookmark(s).", targets.len());
    }
    Ok(())
}

fn run_serve(addr: &str, token_flag: Option<&str>) -> anyhow::Result<()> {
    let (cfg, _) = liber_core::config::load_config()?;
    let token = cfg.resolve_auth_token(token_flag.unwrap_or(""));
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    rt.block_on(liber_server::serve(cfg, token, addr))
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

fn run_check(a: CheckArgs) -> anyhow::Result<()> {
    use liber_core::check::{quarantine_bookmark, CheckStatus};
    use std::time::Duration;

    let (_, mut store) = load_store()?;
    let tokens = match &a.spec {
        Some(s) if !s.trim().is_empty() => Some(liber_core::idspec::parse_id_spec(s)?),
        _ => None,
    };
    let stale = a
        .stale_hours
        .map(|h| Duration::from_secs(h.saturating_mul(3600)));
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
            &o.bookmark.uuid.to_string()[..8],
            o.bookmark.title,
            o.result.target.as_deref().unwrap_or("?"),
            o.result.detail
        );
    }
    for o in &dead {
        println!(
            "[{}] {} dead ({})",
            &o.bookmark.uuid.to_string()[..8],
            o.bookmark.title,
            o.result.detail
        );
    }
    for o in &uncertain {
        println!(
            "[{}] {} uncertain ({})",
            &o.bookmark.uuid.to_string()[..8],
            o.bookmark.title,
            o.result.detail
        );
    }
    if !a.apply {
        return Ok(());
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
                println!("Updated [{}].", &o.bookmark.uuid.to_string()[..8]);
            }
            Err(liber_core::CoreError::Duplicate(_)) => {
                skipped += 1;
                println!(
                    "Skipped [{}]: target URL already bookmarked.",
                    &o.bookmark.uuid.to_string()[..8]
                );
            }
            Err(e) => return Err(e.into()),
        }
    }
    for o in &dead {
        if quarantine_bookmark(&mut store, &o.bookmark.uuid)? {
            quarantined += 1;
            println!("Quarantined [{}].", &o.bookmark.uuid.to_string()[..8]);
        }
    }
    println!("Done: {updated} updated, {quarantined} quarantined, {skipped} skipped.");
    Ok(())
}

fn run_archive(spec: &str, backend: Option<&str>) -> anyhow::Result<()> {
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
                println!("Archived [{}].", &b.uuid.to_string()[..8]);
            }
            Err(e) => println!(
                "warning: archive failed for [{}]: {e}",
                &b.uuid.to_string()[..8]
            ),
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
            println!(
                "Merged {}: {} inserted, {} merged, {} deduped, {} deleted, {} rules.",
                entries.len(),
                rep.inserted,
                rep.merged,
                rep.deduped,
                rep.deleted,
                rep.rules
            );
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

fn run_config(cmd: ConfigCmd) -> anyhow::Result<()> {
    let (mut cfg, path) = liber_core::config::load_config()?;
    match cmd {
        ConfigCmd::Get { key } => match key.as_str() {
            "base_dir" => println!("{}", cfg.base_dir.display()),
            "device_id" => println!("{}", cfg.device_id),
            "archive_backend" => println!("{}", cfg.archive_backend),
            "browser_cmd" => println!("{}", cfg.browser_cmd),
            "browser_path" => println!("{}", cfg.browser_path),
            "singlefile_cmd" => println!("{}", cfg.singlefile_cmd),
            "singlefile_browser_path" => println!("{}", cfg.singlefile_browser_path),
            "monolith_cmd" => println!("{}", cfg.monolith_cmd),
            "auth_token" => println!("{}", cfg.auth_token),
            _ => return Err(anyhow::anyhow!("unknown key {key:?}")),
        },
        ConfigCmd::Set { key, value } => {
            match key.as_str() {
                "base_dir" => cfg.base_dir = value.into(),
                "device_id" => cfg.device_id = value,
                "archive_backend" => match liber_core::archive::parse_backend(&value) {
                    Ok(_) => cfg.archive_backend = value,
                    Err(e) => return Err(anyhow::anyhow!("{e}")),
                },
                "browser_cmd" => cfg.browser_cmd = value,
                "browser_path" => cfg.browser_path = value,
                "singlefile_cmd" => cfg.singlefile_cmd = value,
                "singlefile_browser_path" => cfg.singlefile_browser_path = value,
                "monolith_cmd" => cfg.monolith_cmd = value,
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
            Cmd::Edit(a) => run_edit(a),
            Cmd::Delete(a) => run_delete(&a.spec, a.yes),
            Cmd::Tags(a) => run_tags(a.cmd),
            Cmd::Folders(a) => run_folders(a.cmd),
            Cmd::Auto(a) => run_auto(a.cmd),
            Cmd::Check(a) => run_check(a),
            Cmd::Archive(a) => run_archive(&a.spec, a.backend.as_deref()),
            Cmd::Import(a) => run_import(a),
            Cmd::Export(a) => run_export(a),
            Cmd::Sync(a) => run_sync(a.cmd),
            Cmd::Config(a) => run_config(a.cmd),
            Cmd::Serve(a) => run_serve(&a.addr, a.auth_token.as_deref()),
            _ => {
                println!("not implemented");
                Ok(())
            }
        },
    }
}
