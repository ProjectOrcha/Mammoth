//! The Mammoth CLI. All filesystem operations use the shared Backend boundary.
mod cli;
mod commands;
mod output;
use clap::{CommandFactory, Parser};
use cli::*;
use futures_util::{StreamExt, TryStreamExt};
use mammoth_core::{
    config::{parse_size, Config},
    Backend, Error, Result,
};
use serde_json::json;
use std::{
    io::Write,
    path::{Path, PathBuf},
    sync::Arc,
};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt};
use tokio_util::io::StreamReader;

#[tokio::main]
async fn main() -> std::process::ExitCode {
    let cli = Cli::parse();
    let fmt = cli.format();
    match run(cli).await {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(Error::Io(e)) if e.kind() == std::io::ErrorKind::BrokenPipe => {
            std::process::ExitCode::SUCCESS
        }
        Err(e) => {
            output::print_error(&e, fmt);
            std::process::ExitCode::FAILURE
        }
    }
}
fn default_root() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".mammoth/local")
}
async fn run(cli: Cli) -> Result<()> {
    let fmt = cli.format();
    if cli.masters.len() > 1 {
        return Err(Error::Config(
            "use one HTTP gateway endpoint; multi-master discovery is not implemented".into(),
        ));
    }
    if !cli.masters.is_empty() {
        if let Command::Put { block_size: Some(_), .. } = &cli.command {
            return Err(Error::InvalidInput("per-upload block size over HTTP is unavailable; configure the gateway's storage.block_size".into()));
        }
    }
    if let Command::Version = cli.command {
        return output::emit(
            &json!({"name":"mammoth","version":env!("CARGO_PKG_VERSION"),"backend":"durable local / HTTP gateway"}),
            fmt,
        );
    }
    if let Command::Completions { shell } = &cli.command {
        let shell = shell
            .parse::<clap_complete::Shell>()
            .map_err(|e| Error::InvalidInput(e.to_string()))?;
        clap_complete::generate(shell, &mut Cli::command(), "mammoth", &mut std::io::stdout());
        return Ok(());
    }
    let config = Config::load(cli.config.as_deref())?;
    if let Command::Config { command } = &cli.command {
        return match command {
            ConfigCommand::Show => output::emit(&config, fmt),
            ConfigCommand::Validate => output::emit(&json!({"valid":true}), fmt),
            ConfigCommand::Template => {
                println!(
                    "{}",
                    toml::to_string_pretty(&Config::default())
                        .map_err(|e| Error::Config(e.to_string()))?
                );
                Ok(())
            }
        };
    }
    if let Command::Ui = cli.command {
        let url = format!("http://{}", config.gateway.ui_listen.replace("0.0.0.0", "127.0.0.1"));
        return output::emit(&json!({"url":url,"start":"mammoth quickstart"}), fmt);
    }
    let root = cli.local_root.clone().unwrap_or_else(default_root);
    let mut replication = config.storage.replication;
    let mut block_size = parse_size(&config.storage.block_size)?;
    if let Command::Put { replication: r, block_size: b, .. } = &cli.command {
        if let Some(r) = r {
            replication = *r;
        }
        if let Some(b) = b {
            block_size = parse_size(b)?;
        }
    }
    let backend: Arc<dyn Backend> = if let Some(endpoint) = cli.masters.first() {
        Arc::new(mammoth_client::ClusterBackend::connect(endpoint)?)
    } else {
        let open_root = root.clone();
        let be = tokio::task::spawn_blocking(move || mammoth_local::LocalBackend::open(open_root))
            .await
            .map_err(join_error)??;
        Arc::new(
            be.with_block_size(block_size)
                .with_inline_threshold(parse_size(&config.storage.inline_threshold)?)
                .with_replication(replication),
        )
    };
    let be = backend.as_ref();
    match cli.command {
        Command::Init => {
            let target = root.join("mammoth.toml");
            if !target.exists() {
                let text="[cluster]\nname = \"local\"\n\n[gateway]\nui_listen = \"127.0.0.1:8080\"\ns3_listen = \"127.0.0.1:9000\"\n\n[security]\ntls = \"off\"\nauth = \"none\"\n";
                tokio::fs::write(&target, text).await?;
            }
            output::emit(&json!({"root":root,"config":target}), fmt)
        }
        Command::Quickstart { ui_listen, s3_listen, no_sample, allow_remote } => {
            if !no_sample {
                seed(be).await?;
            }
            eprintln!("{BANNER}");
            local_listeners(&ui_listen, &s3_listen, allow_remote)?;
            mammoth_gateway::serve(backend, &ui_listen, &s3_listen).await
        }
        Command::Serve { role, ui_listen, s3_listen, allow_remote } => {
            if role == "master" || role == "worker" {
                return Err(Error::NotImplemented("separate distributed master/worker roles; run serve --role all for the local service"));
            }
            let ui = ui_listen
                .unwrap_or_else(|| config.gateway.ui_listen.replace("0.0.0.0", "127.0.0.1"));
            let s3 = s3_listen
                .unwrap_or_else(|| config.gateway.s3_listen.replace("0.0.0.0", "127.0.0.1"));
            local_listeners(&ui, &s3, allow_remote)?;
            mammoth_gateway::serve(backend, &ui, &s3).await
        }
        Command::Ls { path } => output::emit(&be.list(&path).await?, fmt),
        Command::Stat { path } => output::emit(&be.stat(&path).await?, fmt),
        Command::Put { src, dst, replication, .. } => {
            if let Some(n) = replication {
                let available = be.cluster_report().await?.nodes.len().min(u8::MAX as usize) as u8;
                if n == 0 {
                    return Err(Error::InvalidInput("replication must be positive".into()));
                }
                if n > available {
                    return Err(Error::NotEnoughWorkers { wanted: n, available });
                }
            }
            be.write(&dst, input(&src).await?).await?;
            if !cli.masters.is_empty() {
                if let Some(n) = replication {
                    be.set_replication(&dst, n).await?;
                }
            }
            output::emit(&be.stat(&dst).await?, fmt)
        }
        Command::Get { src, dst, force } => {
            download(be, &src, &dst, force).await?;
            output::emit(&json!({"saved":dst}), fmt)
        }
        Command::Cat { path } => {
            let mut stream = be.read(&path, 0..u64::MAX).await?;
            let mut out = tokio::io::stdout();
            while let Some(chunk) = stream.next().await {
                out.write_all(&chunk?).await?;
            }
            out.flush().await?;
            Ok(())
        }
        Command::Head { path, lines } => print_lines(be, &path, lines, false).await,
        Command::Tail { path, lines } => print_lines(be, &path, lines, true).await,
        Command::Mkdir { path, parents } => {
            be.mkdir(&path, parents).await?;
            output::emit(&json!({"created":path}), fmt)
        }
        Command::Rm { path, recursive } => {
            be.remove(&path, recursive).await?;
            output::emit(&json!({"removed":path}), fmt)
        }
        Command::Mv { src, dst } => {
            be.rename(&src, &dst).await?;
            output::emit(&json!({"moved":src,"to":dst}), fmt)
        }
        Command::Cp { src, dst, recursive } => {
            copy(be, &src, &dst, recursive).await?;
            output::emit(&json!({"copied":src,"to":dst}), fmt)
        }
        Command::Du { path } => {
            let files = mammoth_gateway::walk(be, &path).await?;
            output::emit(
                &json!({"path":path,"bytes":files.iter().filter(|s|!s.is_dir).map(|s|s.len).sum::<u64>(),"files":files.iter().filter(|s|!s.is_dir).count()}),
                fmt,
            )
        }
        Command::Df | Command::Cluster { .. } => output::emit(&be.cluster_report().await?, fmt),
        Command::Find { path, name } => {
            let all = mammoth_gateway::walk(be, &path).await?;
            output::emit(
                &all.into_iter()
                    .filter(|s| {
                        name.as_ref().is_none_or(|n| {
                            s.path.file_name().unwrap_or_default().to_string_lossy().contains(n)
                        })
                    })
                    .collect::<Vec<_>>(),
                fmt,
            )
        }
        Command::Chmod { mode, path } => {
            let mode = u32::from_str_radix(&mode, 8)
                .map_err(|_| Error::InvalidInput("mode must be octal".into()))?;
            be.set_attributes(&path, Some(mode), None, None).await?;
            output::emit(&be.stat(&path).await?, fmt)
        }
        Command::Chown { owner, path } => {
            let (owner, group) = owner
                .split_once(':')
                .map_or((owner.as_str(), None), |(o, g)| (o, Some(g.to_string())));
            be.set_attributes(&path, None, Some(owner.into()), group).await?;
            output::emit(&be.stat(&path).await?, fmt)
        }
        Command::Setrep { replication, path } => {
            be.set_replication(&path, replication).await?;
            output::emit(&be.stat(&path).await?, fmt)
        }
        Command::Checksum { path } => {
            let mut stream = be.read(&path, 0..u64::MAX).await?;
            let mut crc = 0;
            while let Some(c) = stream.next().await {
                crc = crc32c::crc32c_append(crc, &c?);
            }
            output::emit(&json!({"path":path,"checksum":format!("crc32c:{crc:08x}")}), fmt)
        }
        Command::Viz { what } => viz(backend, what, fmt).await,
        Command::Top { once } => {
            if once {
                output::emit(&be.cluster_report().await?, fmt)
            } else {
                mammoth_viz::top(backend).await
            }
        }
        Command::Node { command } => match command {
            NodeCommand::List => output::emit(&be.cluster_report().await?.nodes, fmt),
            NodeCommand::Inspect { id } => {
                let report = be.cluster_report().await?;
                output::emit(
                    &report
                        .nodes
                        .iter()
                        .find(|n| n.id.0 == id)
                        .ok_or_else(|| Error::NotFound(id.into()))?,
                    fmt,
                )
            }
            NodeCommand::Repair => output::emit(&json!({"repaired":be.repair().await?}), fmt),
        },
        Command::Admin { command } => match command {
            AdminCommand::Report => output::emit(&be.cluster_report().await?, fmt),
            AdminCommand::Repair => output::emit(&json!({"repaired":be.repair().await?}), fmt),
            AdminCommand::Gc => output::emit(&json!({"removed":be.gc().await?}), fmt),
            AdminCommand::Safemode => {
                output::emit(&json!({"safe_mode":be.cluster_report().await?.safe_mode}), fmt)
            }
        },
        Command::Doctor { fix, node } => {
            let repaired = if fix { be.repair().await? } else { 0 };
            let report = be.cluster_report().await?;
            if let Some(id) = node {
                if !report.nodes.iter().any(|n| n.id.0 == id) {
                    return Err(Error::NotFound(id.into()));
                }
            }
            let healthy = report.health.corrupt == 0
                && report.health.missing == 0
                && report.health.critical == 0
                && report.health.under_replicated == 0;
            output::emit(
                &json!({"config_valid":true,"store":root,"healthy":healthy,"repaired":repaired,"health":report.health,"mode":"local; POSIX ownership is descriptive; no distributed HA"}),
                fmt,
            )?;
            if !healthy {
                return Err(Error::InvalidInput(
                    "damaged replicas found; run mammoth admin repair".into(),
                ));
            }
            Ok(())
        }
        Command::Bench { size } => {
            let n = parse_size(&size)?;
            if n > 256 * 1024 * 1024 {
                return Err(Error::InvalidInput(
                    "local benchmark size is limited to 256 MiB".into(),
                ));
            }
            let p = PathBuf::from(format!(
                "/.bench-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_nanos()
            ));
            let start = std::time::Instant::now();
            be.write(&p, mammoth_local::body(vec![0x5a; n as usize])).await?;
            let write_s = start.elapsed().as_secs_f64();
            let start = std::time::Instant::now();
            let mut s = be.read(&p, 0..u64::MAX).await?;
            let mut total = 0;
            while let Some(c) = s.next().await {
                let c = c?;
                if c.iter().any(|v| *v != 0x5a) {
                    return Err(Error::InvalidInput("benchmark checksum failed".into()));
                }
                total += c.len();
            }
            let read_s = start.elapsed().as_secs_f64();
            be.remove(&p, false).await?;
            output::emit(
                &json!({"bytes":total,"write_seconds":write_s,"read_seconds":read_s,"write_mib_s":n as f64/1048576.0/write_s,"read_mib_s":n as f64/1048576.0/read_s,"scope":"local staged I/O"}),
                fmt,
            )
        }
        Command::Job { command } => commands::job(be, command, fmt).await,
        Command::Migrate { command } => commands::migrate(be, command, fmt).await,
        Command::Compat { args } => {
            let args: Vec<_> = args
                .iter()
                .skip_while(|s| s.as_str() == "hdfs" || s.as_str() == "dfs")
                .cloned()
                .collect();
            let cmd = args
                .first()
                .ok_or_else(|| Error::InvalidInput("expected hdfs dfs command".into()))?;
            let translated = match cmd.as_str() {
                "-ls" => "ls",
                "-put" => "put",
                "-get" => "get",
                "-cat" => "cat",
                "-mkdir" => "mkdir",
                "-rm" => "rm",
                "-mv" => "mv",
                "-cp" => "cp",
                "-du" => "du",
                "-df" => "df",
                "-stat" => "stat",
                _ => {
                    return Err(Error::InvalidInput(format!(
                        "unsupported compatibility command: {cmd}"
                    )))
                }
            };
            let mut child = tokio::process::Command::new(std::env::current_exe()?);
            child.arg("--local-root").arg(root);
            if let Some(config) = cli.config {
                child.arg("--config").arg(config);
            }
            for master in cli.masters {
                child.arg("--masters").arg(master);
            }
            if matches!(fmt.resolve(), OutputFormat::Json) {
                child.arg("--json");
            }
            child.arg(translated).args(&args[1..]);
            if !child.status().await?.success() {
                return Err(Error::InvalidInput("translated command failed".into()));
            }
            Ok(())
        }
        Command::Version | Command::Completions { .. } | Command::Config { .. } | Command::Ui => {
            unreachable!("handled before backend creation")
        }
    }
}
fn join_error(e: tokio::task::JoinError) -> Error {
    Error::Io(std::io::Error::other(e))
}
fn local_listeners(ui: &str, s3: &str, allow_remote: bool) -> Result<()> {
    for address in [ui, s3] {
        let address =
            address.parse::<std::net::SocketAddr>().map_err(|e| Error::Config(e.to_string()))?;
        if !address.ip().is_loopback() && !allow_remote {
            return Err(Error::Config(
                "non-loopback listeners require --allow-remote (development only; no authentication)".into(),
            ));
        }
    }
    Ok(())
}
use mammoth_migrate::{download, input};
async fn copy(be: &dyn Backend, src: &Path, dst: &Path, recursive: bool) -> Result<()> {
    let s = be.stat(src).await?;
    let src = s.path.clone();
    let dst = PathBuf::from(mammoth_local::normalize(dst)?);
    if dst == src || dst.starts_with(&src) && s.is_dir {
        return Err(Error::InvalidInput("copy destination must be outside the source".into()));
    }
    if s.is_dir {
        if !recursive {
            return Err(Error::InvalidInput("copying a directory requires --recursive".into()));
        }
        be.mkdir(&dst, true).await?;
        for f in mammoth_gateway::walk(be, &src).await? {
            if f.path == src {
                continue;
            }
            let target = dst
                .join(f.path.strip_prefix(&src).map_err(|e| Error::InvalidInput(e.to_string()))?);
            if f.is_dir {
                be.mkdir(&target, true).await?;
            } else {
                be.write(&target, be.read(&f.path, 0..u64::MAX).await?).await?;
            }
        }
    } else {
        be.write(&dst, be.read(&src, 0..u64::MAX).await?).await?;
    }
    Ok(())
}
async fn print_lines(be: &dyn Backend, path: &Path, lines: usize, tail: bool) -> Result<()> {
    let stream = be.read(path, 0..u64::MAX).await?.map_err(std::io::Error::other);
    let mut reader = tokio::io::BufReader::new(StreamReader::new(stream));
    let mut buf = vec![];
    let mut recent = std::collections::VecDeque::new();
    let mut n = 0;
    let mut out = tokio::io::stdout();
    if lines == 0 {
        return Ok(());
    }
    loop {
        buf.clear();
        if reader.read_until(b'\n', &mut buf).await? == 0 {
            break;
        }
        if tail {
            recent.push_back(buf.clone());
            if recent.len() > lines {
                recent.pop_front();
            }
        } else {
            out.write_all(&buf).await?;
            n += 1;
            if n >= lines {
                break;
            }
        }
    }
    for line in recent {
        out.write_all(&line).await?;
    }
    out.flush().await?;
    Ok(())
}
async fn seed(be: &dyn Backend) -> Result<()> {
    if matches!(be.stat(Path::new("/sample/hello.txt")).await, Err(Error::NotFound(_))) {
        be.write(
            Path::new("/sample/hello.txt"),
            mammoth_local::body(
                b"Hello from Mammoth!\nYour files persist across restarts.\n".to_vec(),
            ),
        )
        .await?;
    }
    if matches!(be.stat(Path::new("/sample/words.txt")).await, Err(Error::NotFound(_))) {
        be.write(
            Path::new("/sample/words.txt"),
            mammoth_local::body(b"mammoth rust storage\nmammoth storage\n".to_vec()),
        )
        .await?;
    }
    if matches!(be.stat(Path::new("/sample/blocks.bin")).await, Err(Error::NotFound(_))) {
        let bytes: Vec<u8> = (0..2 * 1024 * 1024).map(|i| (i % 251) as u8).collect();
        be.write(Path::new("/sample/blocks.bin"), mammoth_local::body(bytes)).await?;
    }
    Ok(())
}
async fn viz(backend: Arc<dyn Backend>, what: VizCommand, fmt: OutputFormat) -> Result<()> {
    let be = backend.as_ref();
    match what {
        VizCommand::Blocks { path } => {
            let layout = be.block_layout(&path).await?;
            if fmt.resolve() == OutputFormat::Table {
                std::io::stdout().write_all(mammoth_viz::blocks(&layout).as_bytes())?;
                Ok(())
            } else {
                output::emit(&layout, fmt)
            }
        }
        VizCommand::Cluster => {
            let report = be.cluster_report().await?;
            if fmt.resolve() == OutputFormat::Table {
                std::io::stdout().write_all(mammoth_viz::cluster(&report).as_bytes())?;
                Ok(())
            } else {
                output::emit(&report, fmt)
            }
        }
        VizCommand::Topology => {
            let report = be.cluster_report().await?;
            let mut racks = std::collections::BTreeMap::<String, Vec<String>>::new();
            for n in report.nodes {
                racks.entry(n.rack).or_default().push(n.id.0);
            }
            output::emit(&racks, fmt)
        }
        VizCommand::Skew { path, by_partition } => {
            let path = path.unwrap_or_else(|| "/".into());
            if by_partition {
                let mut partitions = vec![];
                for s in be.list(&path).await? {
                    let all = mammoth_gateway::walk(be, &s.path).await?;
                    partitions.push(json!({"partition":s.path,"bytes":all.iter().filter(|f|!f.is_dir).map(|f|f.len).sum::<u64>()}));
                }
                output::emit(&partitions, fmt)
            } else {
                output::emit(&mammoth_gateway::skew(be, &path).await?, fmt)
            }
        }
        VizCommand::Treemap { path, depth } => {
            let path = path.unwrap_or_else(|| "/".into());
            let root = be.stat(&path).await?.path;
            let all = mammoth_gateway::walk(be, &root).await?;
            let rows:Vec<_>=all.iter().filter(|s|s.path.components().count().saturating_sub(root.components().count())<=depth as usize).map(|s|json!({"path":s.path,"bytes":if s.is_dir {all.iter().filter(|f|!f.is_dir&&f.path.starts_with(&s.path)).map(|f|f.len).sum::<u64>()}else {s.len}})).collect();
            output::emit(&rows, fmt)
        }
        VizCommand::Health { live } => {
            loop {
                output::emit(&be.cluster_report().await?.health, fmt)?;
                if !live {
                    break;
                }
                tokio::select! {_=tokio::signal::ctrl_c()=>break,_=tokio::time::sleep(std::time::Duration::from_secs(2))=>{}}
            }
            Ok(())
        }
        VizCommand::Flow => output::emit(
            &json!({"available":false,"message":"Network flow measurements are unavailable in the local backend"}),
            fmt,
        ),
    }
}
