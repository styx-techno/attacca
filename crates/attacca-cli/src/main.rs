//! Week-1 protocol spike: prove SOOD discovery, MOO registration/pairing,
//! zone subscription, and transport control against a live Roon Core.
//!
//! Usage:
//!   attacca discover              # raw SOOD sweep: list Cores (host, port, version)
//!   attacca watch                 # pair + stream zone/now-playing events (default)
//!   attacca toggle [ZONE]         # play/pause a zone (substring match), then exit
//!   attacca --host IP --port N …  # skip discovery, connect directly

use attacca_core::{ControlAction, RoonEvent, Zone, ZoneEvent};
use std::time::Duration;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "warn".into()),
        )
        .init();

    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut host: Option<String> = None;
    let mut port: Option<u16> = None;
    let mut command: Vec<String> = Vec::new();
    let mut it = args.into_iter();
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--host" => host = it.next(),
            "--port" => port = it.next().and_then(|p| p.parse().ok()),
            _ => command.push(arg),
        }
    }

    match command.first().map(String::as_str) {
        Some("discover") => discover().await,
        Some("toggle") => {
            let filter = command.get(1).cloned();
            with_core(host, port, |core| async move {
                toggle(core, filter).await
            })
            .await
        }
        None | Some("watch") => {
            with_core(host, port, |core| async move { watch(core).await }).await
        }
        Some(other) => anyhow::bail!("unknown command: {other}"),
    }
}

/// Raw SOOD sweep. Prints every Core response for ~5 seconds, then exits.
/// This is the ground truth for the "which port does the Core listen on" question:
/// the SOOD response advertises the MOO/WebSocket port as `http_port`.
async fn discover() -> anyhow::Result<()> {
    let (discovery, mut cores) = roon_sood::SoodDiscovery::start().await?;
    println!("SOOD query sent (UDP {} / multicast — waiting 5s for responses)…", 9003);

    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    let mut seen = std::collections::HashSet::new();
    loop {
        match tokio::time::timeout_at(deadline, cores.recv()).await {
            Ok(Ok(core)) => {
                if seen.insert(core.core_id.clone()) {
                    println!(
                        "  core \"{}\"  {}:{}  version: {}  id: {}",
                        core.name.as_deref().unwrap_or("?"),
                        core.host,
                        core.http_port,
                        core.display_version.as_deref().unwrap_or("?"),
                        core.core_id,
                    );
                }
            }
            Ok(Err(_)) | Err(_) => break,
        }
    }
    discovery.stop().await;
    if seen.is_empty() {
        println!("No Roon Core found. Is one running on this network?");
    }
    Ok(())
}

/// Build the client, obtain a paired Core (via discovery or --host/--port),
/// then run `f` with it.
async fn with_core<F, Fut>(host: Option<String>, port: Option<u16>, f: F) -> anyhow::Result<()>
where
    F: FnOnce(attacca_core::Core) -> Fut,
    Fut: std::future::Future<Output = anyhow::Result<()>>,
{
    println!("Token store: {}", attacca_core::cli_token_store_path().display());
    let client = attacca_core::build_cli_client()?;
    let mut events = client.events();

    if let (Some(host), Some(port)) = (host, port) {
        println!("Connecting to {host}:{port}…");
        let core = client.connect(&host, port).await?;
        return f(core).await;
    }

    println!("Discovering Roon Core… (first run: approve \"Attacca CLI\" in Roon Settings → Extensions)");
    client.start_discovery().await?;
    loop {
        match events.recv().await? {
            RoonEvent::CoreFound { core_id, display_name } => {
                println!("  found core \"{display_name}\" ({core_id}) — registering…");
            }
            RoonEvent::CorePaired(core) => {
                println!("  paired with \"{}\"", core.display_name());
                return f(core).await;
            }
            RoonEvent::CoreLost { core_id } => println!("  lost core {core_id}"),
            RoonEvent::CoreUnpaired { core_id } => println!("  unpaired from {core_id}"),
        }
    }
}

/// Subscribe to zones and stream events until Ctrl-C.
async fn watch(core: attacca_core::Core) -> anyhow::Result<()> {
    let transport = core.transport();
    let mut zones = transport.subscribe_zones().await?;

    loop {
        tokio::select! {
            _ = tokio::signal::ctrl_c() => break,
            event = zones.recv() => {
                let Some(event) = event else { break };
                match event {
                    ZoneEvent::Initial(zones) => {
                        println!("── {} zone(s) ──", zones.len());
                        zones.iter().for_each(print_zone);
                    }
                    ZoneEvent::Changed(zones) => {
                        for z in &zones {
                            let np = z
                                .now_playing
                                .as_ref()
                                .map(|np| np.one_line.line1.as_str())
                                .unwrap_or("–");
                            println!("[{}] {:?}  {}", z.display_name, z.state, np);
                        }
                    }
                    ZoneEvent::Added(zones) => zones.iter().for_each(print_zone),
                    ZoneEvent::Removed(ids) => {
                        ids.iter().for_each(|id| println!("[removed] {id}"))
                    }
                    ZoneEvent::Seeked(_) => {} // too chatty for the spike
                }
            }
        }
    }
    Ok(())
}

/// Play/pause the first zone whose name contains `filter` (or the first zone).
async fn toggle(core: attacca_core::Core, filter: Option<String>) -> anyhow::Result<()> {
    let transport = core.transport();
    let mut zones = transport.subscribe_zones().await?;

    while let Some(event) = zones.recv().await {
        if let ZoneEvent::Initial(zones) = event {
            let zone = zones
                .iter()
                .find(|z| match &filter {
                    Some(f) => z.display_name.to_lowercase().contains(&f.to_lowercase()),
                    None => true,
                })
                .ok_or_else(|| anyhow::anyhow!("no zone matching {filter:?}"))?;
            println!("PlayPause → \"{}\" (was {:?})", zone.display_name, zone.state);
            transport.control(&zone.zone_id, ControlAction::PlayPause).await?;
            tokio::time::sleep(Duration::from_millis(750)).await;
            return Ok(());
        }
    }
    Ok(())
}

fn print_zone(zone: &Zone) {
    println!("  {}  [{:?}]", zone.display_name, zone.state);
    for output in &zone.outputs {
        let vol = output
            .volume
            .as_ref()
            .map(|v| format!("  vol {}/{}", v.value, v.max))
            .unwrap_or_default();
        println!("    ↳ {} ({}){vol}", output.display_name, output.output_id);
        println!("      can group with: {:?}", output.can_group_with_output_ids);
    }
    if let Some(np) = &zone.now_playing {
        println!("    ♪ {}", np.one_line.line1);
    }
}
