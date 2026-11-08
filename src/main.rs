
use anyhow::{bail, Context, Result};
use clap::{ArgGroup, Parser};
use futures_util::StreamExt;
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use reqwest::{Client, StatusCode};
use sha2::{Digest, Sha256};
use std::{
    path::PathBuf,
    process::{Command, Output, Stdio},
    time::{Duration, Instant},
};
use tokio::{
    fs::{self, File},
    io::AsyncWriteExt,
    task::JoinSet,
    time::sleep,
};

const MAX_PARALLEL: usize = 8;
const INITIAL_PARALLEL: usize = 2;
const MAX_RETRIES: usize = 3;

#[derive(Parser)]
#[command(
    name = "drin",
    version,
    about = "DRin - Durian Package Manager",
    group(
        ArgGroup::new("operation")
            .multiple(false)
            .args([
                "update",
                "upgrade",
                "install",
                "remove",
                "search",
                "info",
                "clean",
                "download"
            ])
    )
)]
struct Cli {
    /// Update repository information
    #[arg(short = 'u')]
    update: bool,

    /// Upgrade installed packages
    #[arg(short = 'g')]
    upgrade: bool,

    /// Install one or more packages
    #[arg(short = 'i', num_args = 1..)]
    install: Vec<String>,

    /// Remove one or more packages
    #[arg(short = 'r', num_args = 1..)]
    remove: Vec<String>,

    /// Search for packages
    #[arg(short = 's', num_args = 1..)]
    search: Vec<String>,

    /// Display package information
    #[arg(short = 'f', num_args = 1..)]
    info: Vec<String>,

    /// Clean the package cache
    #[arg(short = 'c')]
    clean: bool,

    /// Download URL=DESTINATION pairs
    #[arg(long = "download", value_name = "URL=DEST", num_args = 1..)]
    download: Vec<String>,

    /// Expected SHA-256 for a single download
    #[arg(long)]
    sha256: Option<String>,

    /// Show detailed command output
    #[arg(short = 'v', long, global = true)]
    verbose: bool,
}

fn header(verbose: bool) {
    println!("\n🍈 DRin 0.3.0");
    println!("Durian Package Manager");

    if verbose {
        println!("Verbose mode: enabled");
    }

    println!();
}

fn is_root() -> bool {
    Command::new("id")
        .arg("-u")
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim() == "0")
        .unwrap_or(false)
}

fn capture_command(
    program: &str,
    args: &[String],
    verbose: bool,
) -> Result<Output> {
    let output = Command::new(program)
        .args(args)
        .output()
        .with_context(|| format!("Could not execute {program}"))?;

    if verbose {
        if !output.stdout.is_empty() {
            print!("{}", String::from_utf8_lossy(&output.stdout));
        }

        if !output.stderr.is_empty() {
            eprint!("{}", String::from_utf8_lossy(&output.stderr));
        }
    }

    Ok(output)
}

fn run_apt_interactive(args: &[String], verbose: bool) -> Result<()> {
    println!("  Running APT...\n");

    let mut command = if is_root() {
        Command::new("apt-get")
    } else {
        let mut cmd = Command::new("sudo");
        cmd.arg("apt-get");
        cmd
    };

    command.args(args);

    if !verbose {
        command.arg("-q");
    }

    // Keep APT's terminal output visible for interactive confirmation
    // and package transaction progress.
    let status = command
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()?;

    if status.success() {
        println!("\n  ✓ Operation completed successfully.");
        Ok(())
    } else {
        bail!("APT failed with status {status}");
    }
}

fn run_apt_quiet(args: &[String], verbose: bool) -> Result<()> {
    println!("  Working...");

    let mut command_args = args.to_vec();

    if !verbose {
        command_args.insert(0, "-q".to_string());
    }

    let output = if is_root() {
        capture_command("apt-get", &command_args, verbose)?
    } else {
        let mut sudo_args = vec!["apt-get".to_string()];
        sudo_args.extend(command_args);

        capture_command("sudo", &sudo_args, verbose)?
    };

    if output.status.success() {
        println!("  ✓ Operation completed successfully.");
        Ok(())
    } else {
        if !verbose && !output.stderr.is_empty() {
            eprintln!("{}", String::from_utf8_lossy(&output.stderr));
        }

        bail!("APT failed with status {}", output.status);
    }
}

fn spinner(message: &str) -> ProgressBar {
    let bar = ProgressBar::new_spinner();

    bar.set_style(
        ProgressStyle::with_template(
            "{spinner:.cyan} {msg}"
        ).unwrap()
    );

    bar.set_message(message.to_string());
    bar.enable_steady_tick(Duration::from_millis(100));

    bar
}

fn handle_search(terms: &[String], verbose: bool) -> Result<()> {
    let start = Instant::now();

    println!("[ SEARCH ] {}", terms.join(" "));

    let bar = spinner("Searching package database...");

    let mut args = vec!["search".to_string()];
    args.extend(terms.iter().cloned());

    let output = capture_command("apt-cache", &args, verbose)?;

    bar.finish_and_clear();

    if !output.status.success() {
        bail!("Package search failed.");
    }

    let results = String::from_utf8_lossy(&output.stdout);

    if verbose {
        println!("{results}");
    } else {
        let entries: Vec<&str> = results
            .lines()
            .filter(|line| !line.trim().is_empty())
            .collect();

        println!("\nFound {} matching results:\n", entries.len());

        for entry in entries.iter().take(30) {
            if let Some((name, description)) = entry.split_once(" - ") {
                println!("  {:<28} {}", name, description);
            } else {
                println!("  {entry}");
            }
        }

        if entries.len() > 30 {
            println!(
                "\n  ...and {} more results.",
                entries.len() - 30
            );
        }
    }

    println!(
        "\nSearch completed in {:.2}s.",
        start.elapsed().as_secs_f64()
    );

    Ok(())
}

fn handle_info(packages: &[String], verbose: bool) -> Result<()> {
    for package in packages {
        println!("[ PACKAGE INFO ] {package}");

        let bar = spinner("Retrieving package information...");

        let output = capture_command(
            "apt-cache",
            &["show".to_string(), package.clone()],
            verbose,
        )?;

        bar.finish_and_clear();

        if !output.status.success() {
            bail!("Could not retrieve information for {package}");
        }

        let data = String::from_utf8_lossy(&output.stdout);

        if verbose {
            println!("{data}");
            continue;
        }

        // apt-cache show can contain multiple versions.
        // Default to the first available version.
        let first_version = data.split("\n\n").next().unwrap_or("");

        let fields = [
            "Package:",
            "Version:",
            "Architecture:",
            "Section:",
            "Priority:",
            "Maintainer:",
            "Depends:",
            "Pre-Depends:",
            "Recommends:",
            "Suggests:",
            "Installed-Size:",
            "Size:",
            "Filename:",
            "SHA256:",
            "Origin:",
            "Description-en:",
        ];

        println!();

        for line in first_version.lines() {
            if fields.iter().any(|field| line.starts_with(field)) {
                println!("  {line}");
            }
        }

        if first_version.trim().is_empty() {
            println!("  No package information found.");
        }

        println!();
    }

    Ok(())
}

fn handle_package_commands(cli: &Cli) -> Result<()> {
    header(cli.verbose);

    if cli.update {
        println!("[ REPOSITORY UPDATE ]\n");

        run_apt_quiet(
            &["update".to_string()],
            cli.verbose,
        )?;
    } else if cli.upgrade {
        println!("[ SYSTEM UPGRADE ]\n");

        run_apt_interactive(
            &["upgrade".to_string()],
            cli.verbose,
        )?;
    } else if !cli.install.is_empty() {
        println!(
            "[ INSTALLATION ] {}\n",
            cli.install.join(", ")
        );

        let mut args = vec!["install".to_string()];
        args.extend(cli.install.iter().cloned());

        run_apt_interactive(&args, cli.verbose)?;
    } else if !cli.remove.is_empty() {
        println!(
            "[ REMOVAL ] {}\n",
            cli.remove.join(", ")
        );

        let mut args = vec!["remove".to_string()];
        args.extend(cli.remove.iter().cloned());

        run_apt_interactive(&args, cli.verbose)?;
    } else if !cli.search.is_empty() {
        handle_search(&cli.search, cli.verbose)?;
    } else if !cli.info.is_empty() {
        handle_info(&cli.info, cli.verbose)?;
    } else if cli.clean {
        println!("[ CACHE CLEANUP ]\n");

        run_apt_quiet(
            &["clean".to_string()],
            cli.verbose,
        )?;
    } else {
        println!("Use 'drin --help' to see available commands.");
    }

    Ok(())
}

// ---------------- ADAPTIVE DOWNLOADER ----------------

#[derive(Clone)]
struct DownloadTask {
    url: String,
    destination: PathBuf,
    expected_sha256: Option<String>,
}

struct DownloadOutcome {
    url: String,
    result: Result<()>,
    throttled: bool,
}

async fn download_many(
    tasks: Vec<DownloadTask>,
    verbose: bool,
) -> Result<()> {
    if tasks.is_empty() {
        bail!("No downloads specified.");
    }

    let client = Client::builder()
        .user_agent("DRin/0.3.0")
        .connect_timeout(Duration::from_secs(15))
        .timeout(Duration::from_secs(300))
        .pool_idle_timeout(Duration::from_secs(90))
        .build()?;

    let total = tasks.len();
    let mut pending = tasks.into_iter();
    let mut active = JoinSet::new();

    let mut concurrency = INITIAL_PARALLEL.min(MAX_PARALLEL);
    let mut healthy = 0usize;
    let mut completed = 0usize;

    let progress = MultiProgress::new();

    println!(
        "Adaptive downloader: {concurrency} initial connections, maximum {MAX_PARALLEL}"
    );

    loop {
        while active.len() < concurrency {
            let Some(task) = pending.next() else {
                break;
            };

            let client = client.clone();
            let progress = progress.clone();

            active.spawn(async move {
                download_one(client, task, progress, verbose).await
            });
        }

        let Some(joined) = active.join_next().await else {
            break;
        };

        let outcome = joined.context("Download worker failed")?;
        completed += 1;

        match outcome.result {
            Ok(()) => {
                println!(
                    "[{completed}/{total}] ✓ {}",
                    outcome.url
                );

                if outcome.throttled {
                    concurrency = (concurrency / 2).max(1);
                    healthy = 0;

                    if verbose {
                        println!(
                            "  Server/network pushback. Concurrency reduced to {concurrency}."
                        );
                    }
                } else {
                    healthy += 1;

                    if healthy >= 2 && concurrency < MAX_PARALLEL {
                        concurrency += 1;
                        healthy = 0;

                        if verbose {
                            println!(
                                "  Healthy downloads. Concurrency increased to {concurrency}."
                            );
                        }
                    }
                }
            }

            Err(error) => {
                active.abort_all();
                bail!("Download failed: {error:#}");
            }
        }
    }

    println!("\n✓ All {total} downloads completed.");

    Ok(())
}

async fn download_one(
    client: Client,
    task: DownloadTask,
    progress: MultiProgress,
    verbose: bool,
) -> DownloadOutcome {
    let mut throttled = false;

    let result = download_with_retries(
        &client,
        &task,
        &progress,
        &mut throttled,
        verbose,
    )
    .await;

    DownloadOutcome {
        url: task.url,
        result,
        throttled,
    }
}

async fn download_with_retries(
    client: &Client,
    task: &DownloadTask,
    progress: &MultiProgress,
    throttled: &mut bool,
    verbose: bool,
) -> Result<()> {
    let parent = task.destination
        .parent()
        .unwrap_or_else(|| std::path::Path::new("."));

    fs::create_dir_all(parent).await?;

    let temp = task.destination.with_extension("drin-part");

    for attempt in 0..=MAX_RETRIES {
        if verbose && attempt > 0 {
            println!(
                "Retry {attempt}/{MAX_RETRIES}: {}",
                task.url
            );
        }

        let response = match client.get(&task.url).send().await {
            Ok(response) if response.status().is_success() => response,

            Ok(response) => {
                let status = response.status();

                if retryable(status) && attempt < MAX_RETRIES {
                    *throttled = true;
                    sleep(backoff(attempt)).await;
                    continue;
                }

                bail!("HTTP error: {status}");
            }

            Err(error)
                if (error.is_timeout() || error.is_connect())
                    && attempt < MAX_RETRIES =>
            {
                *throttled = true;
                sleep(backoff(attempt)).await;
                continue;
            }

            Err(error) => return Err(error.into()),
        };

        let length = response.content_length().unwrap_or(0);

        let bar = if length > 0 {
            ProgressBar::new(length)
        } else {
            ProgressBar::new_spinner()
        };

        let style = if length > 0 {
            ProgressStyle::with_template(
                "{msg} [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({bytes_per_sec})"
            )?
        } else {
            ProgressStyle::with_template(
                "{spinner:.green} {msg} {bytes} ({bytes_per_sec})"
            )?
        };

        bar.set_style(style);
        bar.set_message(task.destination.display().to_string());
        progress.add(bar.clone());

        let mut stream = response.bytes_stream();
        let mut file = File::create(&temp).await?;
        let mut hasher = Sha256::new();

        let result: Result<()> = async {
            while let Some(chunk) = stream.next().await {
                let chunk = chunk?;
                hasher.update(&chunk);
                file.write_all(&chunk).await?;
                bar.inc(chunk.len() as u64);
            }

            file.flush().await?;
            Ok(())
        }
        .await;

        drop(file);

        if let Err(error) = result {
            bar.abandon_with_message("Download interrupted");
            let _ = fs::remove_file(&temp).await;

            if attempt < MAX_RETRIES {
                *throttled = true;
                sleep(backoff(attempt)).await;
                continue;
            }

            return Err(error);
        }

        if let Some(expected) = &task.expected_sha256 {
            let actual = hex::encode(hasher.finalize());

            if !actual.eq_ignore_ascii_case(expected) {
                let _ = fs::remove_file(&temp).await;
                bar.abandon_with_message("Checksum mismatch");
                bail!("SHA-256 verification failed.");
            }
        }

        fs::rename(&temp, &task.destination).await?;
        bar.finish_with_message("Complete");

        return Ok(());
    }

    bail!("Download retry limit reached.")
}

fn retryable(status: StatusCode) -> bool {
    status == StatusCode::TOO_MANY_REQUESTS
        || status == StatusCode::BAD_GATEWAY
        || status == StatusCode::SERVICE_UNAVAILABLE
        || status == StatusCode::GATEWAY_TIMEOUT
}

fn backoff(attempt: usize) -> Duration {
    Duration::from_secs(1u64 << attempt.min(4))
}

// ---------------- MAIN ----------------

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    if !cli.download.is_empty() {
        if cli.sha256.is_some() && cli.download.len() != 1 {
            bail!("--sha256 can only be used with one download.");
        }

        header(cli.verbose);

        let mut tasks = Vec::new();

        for (index, item) in cli.download.iter().enumerate() {
            let (url, destination) = item
                .split_once('=')
                .context("Expected URL=DESTINATION")?;

            if url.is_empty() || destination.is_empty() {
                bail!("Invalid download argument: {item}");
            }

            tasks.push(DownloadTask {
                url: url.to_string(),
                destination: destination.into(),
                expected_sha256: if index == 0 {
                    cli.sha256.clone()
                } else {
                    None
                },
            });
        }

        download_many(tasks, cli.verbose).await?;
    } else {
        handle_package_commands(&cli)?;
    }

    Ok(())
}
