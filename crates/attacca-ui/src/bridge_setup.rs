//! Guided Roon Bridge setup.
//!
//! Roon's terms do not permit bundling Roon Bridge, so this downloads it from
//! Roon's own servers on the user's request (the pattern the AUR roonbridge
//! package established), installs it under ~/.local/share/attacca/roonbridge,
//! and runs it as a per-user systemd service. Running as the desktop user —
//! not root like Roon's easy installer — is what makes the per-device
//! "desktop mix" mode possible: RAATServer can then open the `pipewire` ALSA
//! PCM, which lives in the user session.

#[cxx_qt::bridge]
pub mod qsetup {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
    }

    extern "RustQt" {
        #[qobject]
        #[qml_element]
        #[qproperty(bool, busy)]
        #[qproperty(f64, progress)]
        #[qproperty(QString, status_text, cxx_name = "statusText")]
        #[qproperty(QString, installed_version, cxx_name = "installedVersion")]
        #[qproperty(bool, service_active, cxx_name = "serviceActive")]
        #[qproperty(bool, raat_alive, cxx_name = "raatAlive")]
        #[qproperty(bool, pipewire_ok, cxx_name = "pipewireOk")]
        #[qproperty(bool, sandboxed)]
        type BridgeSetup = super::BridgeSetupRust;

        /// Enabled devices as a JSON array of {id, name, device, mode}.
        #[qsignal]
        fn devices(self: Pin<&mut BridgeSetup>, json: QString);

        #[qsignal]
        #[cxx_name = "opFinished"]
        fn op_finished(self: Pin<&mut BridgeSetup>, ok: bool, message: QString);

        #[qinvokable]
        fn refresh(self: Pin<&mut Self>);

        #[qinvokable]
        #[cxx_name = "scanDevices"]
        fn scan_devices(self: Pin<&mut Self>);

        #[qinvokable]
        fn install(self: Pin<&mut Self>);

        #[qinvokable]
        fn uninstall(self: Pin<&mut Self>);

        #[qinvokable]
        #[cxx_name = "setDeviceMode"]
        fn set_device_mode(self: Pin<&mut Self>, device_id: &QString, mode: &QString);
    }

    impl cxx_qt::Threading for BridgeSetup {}
}

use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::process::Command;
use std::time::Duration;

use anyhow::{bail, Context};
use cxx_qt::Threading;
use cxx_qt_lib::QString;

use self::qsetup::BridgeSetup;

type QtThread = cxx_qt::CxxQtThread<BridgeSetup>;

pub struct BridgeSetupRust {
    busy: bool,
    progress: f64,
    status_text: QString,
    installed_version: QString,
    service_active: bool,
    raat_alive: bool,
    pipewire_ok: bool,
    sandboxed: bool,
}

impl Default for BridgeSetupRust {
    fn default() -> Self {
        Self {
            busy: false,
            progress: -1.0,
            status_text: QString::default(),
            installed_version: QString::default(),
            service_active: false,
            raat_alive: false,
            pipewire_ok: false,
            sandboxed: false,
        }
    }
}

const UNIT_NAME: &str = "attacca-roonbridge.service";
/// RAATServer's local control port; a Bridge helper that finds it occupied
/// attaches to the existing RAATServer instead of starting its own, so an
/// answer here means "a Bridge already serves this machine's audio devices" —
/// whoever owns the process.
const RAAT_PORT: u16 = 9004;

fn base_dir() -> Option<PathBuf> {
    Some(dirs_next::data_dir()?.join("attacca").join("roonbridge"))
}

fn program_dir() -> Option<PathBuf> {
    Some(base_dir()?.join("RoonBridge"))
}

fn data_dir() -> Option<PathBuf> {
    Some(base_dir()?.join("data"))
}

fn unit_path() -> Option<PathBuf> {
    Some(
        dirs_next::config_dir()?
            .join("systemd")
            .join("user")
            .join(UNIT_NAME),
    )
}

fn settings_dir() -> Option<PathBuf> {
    Some(data_dir()?.join("RAATServer").join("Settings"))
}

fn push<F>(qt: &QtThread, f: F)
where
    F: FnOnce(Pin<&mut BridgeSetup>) + Send + 'static,
{
    let _ = qt.queue(f);
}

fn status(qt: &QtThread, text: &str, progress: f64) {
    let text = text.to_owned();
    push(qt, move |mut o| {
        o.as_mut().set_status_text(QString::from(&text));
        o.as_mut().set_progress(progress);
    });
}

fn systemctl(args: &[&str]) -> anyhow::Result<String> {
    let out = Command::new("systemctl")
        .arg("--user")
        .args(args)
        .output()
        .context("systemd is required (systemctl not found)")?;
    let stdout = String::from_utf8_lossy(&out.stdout).trim().to_owned();
    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr).trim().to_owned();
        bail!(
            "systemctl --user {} failed: {}",
            args.join(" "),
            if stderr.is_empty() { &stdout } else { &stderr }
        );
    }
    Ok(stdout)
}

fn service_is_active() -> bool {
    Command::new("systemctl")
        .args(["--user", "is-active", UNIT_NAME])
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim() == "active")
        .unwrap_or(false)
}

fn raat_answers() -> bool {
    let addr = std::net::SocketAddr::from(([127, 0, 0, 1], RAAT_PORT));
    std::net::TcpStream::connect_timeout(&addr, Duration::from_millis(300)).is_ok()
}

fn pipewire_alsa_present() -> bool {
    // The pipewire ALSA PCM comes from pipewire-alsa's config drop-in; the
    // library itself is dlopen'd, so the config file is the reliable marker.
    [
        "/usr/share/alsa/alsa.conf.d/50-pipewire.conf",
        "/etc/alsa/conf.d/50-pipewire.conf",
    ]
    .iter()
    .any(|p| Path::new(p).exists())
}

fn in_flatpak() -> bool {
    Path::new("/.flatpak-info").exists() || std::env::var_os("FLATPAK_ID").is_some()
}

fn installed_version() -> String {
    let Some(dir) = program_dir() else {
        return String::new();
    };
    match std::fs::read_to_string(dir.join("VERSION")) {
        // Line 2 is the human-readable one: "2.71 (build 1683) production".
        Ok(v) => v.lines().nth(1).unwrap_or_default().trim().to_owned(),
        Err(_) => String::new(),
    }
}

/// Read every enabled device's RAATServer settings file into the JSON shape
/// the QML device list consumes.
fn device_list_json() -> String {
    let mut devices = Vec::new();
    if let Some(dir) = settings_dir() {
        let entries = std::fs::read_dir(&dir).into_iter().flatten().flatten();
        for entry in entries {
            let name = entry.file_name().to_string_lossy().into_owned();
            let Some(id) = name
                .strip_prefix("device_")
                .and_then(|s| s.strip_suffix(".json"))
            else {
                continue;
            };
            let Ok(text) = std::fs::read_to_string(entry.path()) else {
                continue;
            };
            let Ok(json) = serde_json::from_str::<serde_json::Value>(&text) else {
                continue;
            };
            let device = json["output"]["device"].as_str().unwrap_or_default();
            let display = json["output"]["name"].as_str().unwrap_or(device);
            devices.push(serde_json::json!({
                "id": id,
                "name": display,
                "device": device,
                "mode": if device == "plug:pipewire" { "pipewire" } else { "exclusive" },
            }));
        }
    }
    devices.sort_by(|a, b| a["name"].as_str().cmp(&b["name"].as_str()));
    serde_json::Value::Array(devices).to_string()
}

/// Sync all detection-derived properties; runs on the Qt thread.
fn apply_detection(mut o: Pin<&mut BridgeSetup>) {
    o.as_mut()
        .set_installed_version(QString::from(&installed_version()));
    o.as_mut().set_service_active(service_is_active());
    o.as_mut().set_raat_alive(raat_answers());
    o.as_mut().set_pipewire_ok(pipewire_alsa_present());
    o.as_mut().set_sandboxed(in_flatpak());
}

/// Spawn `work` on its own thread with the busy flag held; its Ok/Err becomes
/// the opFinished signal and detection is re-run either way.
fn run_op(
    o: &mut Pin<&mut BridgeSetup>,
    work: impl FnOnce(&QtThread) -> anyhow::Result<String> + Send + 'static,
) {
    if *o.busy() {
        return;
    }
    o.as_mut().set_busy(true);
    o.as_mut().set_progress(-1.0);
    let qt = o.qt_thread();
    std::thread::spawn(move || {
        // A panic here would skip the completion closure and latch busy=true
        // forever (every wizard button is gated on it), so it must become an
        // ordinary failed result instead.
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| work(&qt)))
            .unwrap_or_else(|_| Err(anyhow::anyhow!("internal error (panic) — restart Attacca and try again")));
        push(&qt, move |mut o| {
            apply_detection(o.as_mut());
            o.as_mut().set_busy(false);
            o.as_mut().set_progress(-1.0);
            o.as_mut().set_status_text(QString::default());
            let json = device_list_json();
            o.as_mut().devices(QString::from(&json));
            let (ok, message) = match result {
                Ok(m) => (true, m),
                Err(e) => (false, format!("{e:#}")),
            };
            o.as_mut().op_finished(ok, QString::from(&message));
        });
    });
}

impl qsetup::BridgeSetup {
    pub fn refresh(mut self: Pin<&mut Self>) {
        // Detection is a few file reads, one systemctl call and one localhost
        // connect with a short timeout — cheap enough for the UI thread.
        apply_detection(self.as_mut());
    }

    pub fn scan_devices(mut self: Pin<&mut Self>) {
        let json = device_list_json();
        self.as_mut().devices(QString::from(&json));
    }

    pub fn install(mut self: Pin<&mut Self>) {
        run_op(&mut self, do_install);
    }

    pub fn uninstall(mut self: Pin<&mut Self>) {
        run_op(&mut self, |_| do_uninstall());
    }

    pub fn set_device_mode(mut self: Pin<&mut Self>, device_id: &QString, mode: &QString) {
        let id = device_id.to_string();
        let mode = mode.to_string();
        run_op(&mut self, move |qt| do_set_device_mode(qt, &id, &mode));
    }
}

fn tarball_url() -> anyhow::Result<&'static str> {
    match std::env::consts::ARCH {
        "x86_64" => Ok("https://download.roonlabs.net/builds/RoonBridge_linuxx64.tar.bz2"),
        "aarch64" => Ok("https://download.roonlabs.net/builds/RoonBridge_linuxarmv8.tar.bz2"),
        "arm" => Ok("https://download.roonlabs.net/builds/RoonBridge_linuxarmv7hf.tar.bz2"),
        other => bail!("Roon Bridge is not available for the {other} architecture"),
    }
}

fn download(qt: &QtThread, url: &str, dest: &Path) -> anyhow::Result<()> {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    rt.block_on(async {
        // Per-read timeout, not a whole-request one: the tarball is tens of
        // MB and a slow link must not be cut off, but a stalled connection
        // would otherwise hold `busy` forever (there is no cancel path).
        let client = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(15))
            .read_timeout(Duration::from_secs(30))
            .build()?;
        let resp = client.get(url).send().await?.error_for_status()?;
        let total = resp.content_length();
        let mut file = std::fs::File::create(dest)?;
        let mut got: u64 = 0;
        let mut last_pct: i64 = -1;
        let mut resp = resp;
        while let Some(chunk) = resp.chunk().await? {
            file.write_all(&chunk)?;
            got += chunk.len() as u64;
            if let Some(total) = total.filter(|t| *t > 0) {
                let pct = (got * 100 / total) as i64;
                if pct != last_pct {
                    last_pct = pct;
                    status(
                        qt,
                        &format!("Downloading Roon Bridge… {pct}%"),
                        got as f64 / total as f64,
                    );
                }
            }
        }
        file.sync_all()?;
        anyhow::Ok(())
    })
}

/// Paths land verbatim inside systemd directives, where `%` is a specifier
/// and quotes, backslashes and control characters change parsing — quote the
/// value, escape `%`, and refuse what cannot be carried rather than writing a
/// unit that means something else than intended.
fn unit_path_str(path: &Path) -> anyhow::Result<String> {
    let s = path.to_str().context("install path is not valid UTF-8")?;
    if s.chars().any(|c| c.is_control() || c == '"' || c == '\\') {
        bail!("install path contains characters a systemd unit cannot carry: {s:?}");
    }
    Ok(s.replace('%', "%%"))
}

fn do_install(qt: &QtThread) -> anyhow::Result<String> {
    if in_flatpak() {
        bail!("the Flatpak sandbox cannot install services on the host");
    }
    // A Bridge we don't manage already owns this machine's RAATServer; ours
    // would silently attach to it and serve nothing, then the success check
    // below would pass on the foreign instance's port.
    if raat_answers() && !service_is_active() {
        bail!(
            "another Roon Bridge already serves this computer's audio devices — \
             remove it first (for example a system-wide roonbridge service)"
        );
    }
    let base = base_dir().context("no XDG data directory")?;
    std::fs::create_dir_all(&base)?;
    // Transients from crashed or failed earlier runs are only useful to a
    // retry that overwrites them anyway.
    cleanup_transients(&base);
    let staging = base.join(format!(".extract.tmp-{}", std::process::id()));
    let tarball = base.join("RoonBridge.tar.bz2.part");
    let result = install_inner(qt, &base, &staging, &tarball);
    cleanup_transients(&base);
    result
}

fn cleanup_transients(base: &Path) {
    for entry in std::fs::read_dir(base).into_iter().flatten().flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.starts_with(".extract.tmp") || name.starts_with("RoonBridge.old") {
            let _ = std::fs::remove_dir_all(entry.path());
        } else if name == "RoonBridge.tar.bz2.part" {
            let _ = std::fs::remove_file(entry.path());
        }
    }
}

fn install_inner(
    qt: &QtThread,
    base: &Path,
    staging: &Path,
    tarball: &Path,
) -> anyhow::Result<String> {
    let program = program_dir().unwrap();
    let data = data_dir().unwrap();
    let unit_file = unit_path().context("no XDG config directory")?;
    std::fs::create_dir_all(&data)?;

    status(qt, "Downloading Roon Bridge…", -1.0);
    let url = tarball_url()?;
    download(qt, url, tarball).context("download failed")?;

    status(qt, "Unpacking…", -1.0);
    std::fs::create_dir_all(staging)?;
    let out = Command::new("tar")
        .arg("xjf")
        .arg(tarball)
        .arg("-C")
        .arg(staging)
        .output()
        .context("tar is required")?;
    if !out.status.success() {
        bail!("unpack failed: {}", String::from_utf8_lossy(&out.stderr));
    }
    let unpacked = staging.join("RoonBridge");
    if !unpacked.join("start.sh").exists() {
        bail!("unexpected archive layout (no RoonBridge/start.sh)");
    }

    status(qt, "Checking system requirements…", -1.0);
    // Roon's own dependency check; exit code 3 is its "definitely missing a
    // required library" verdict, anything else is best-effort advisory.
    let check = Command::new(unpacked.join("check.sh"))
        .arg("--preflight")
        .output();
    if let Ok(out) = check {
        if out.status.code() == Some(3) {
            bail!(
                "this system is missing a library Roon Bridge needs: {}",
                String::from_utf8_lossy(&out.stdout).trim()
            );
        }
    }

    status(qt, "Installing…", -1.0);
    // An update replaces the program directory while the data directory (the
    // Bridge's identity and device settings) stays, so zones survive updates.
    // The old tree is renamed aside, not deleted in place: deleting hundreds
    // of files takes long enough that a crash would leave the enabled unit
    // pointing at nothing, while a rename-rename swap window is two syscalls.
    if unit_file.exists() {
        let _ = systemctl(&["stop", UNIT_NAME]);
    }
    let old = base.join(format!("RoonBridge.old-{}", std::process::id()));
    if program.exists() {
        std::fs::rename(&program, &old).context("could not move the previous install aside")?;
    }
    std::fs::rename(&unpacked, &program)?;
    let _ = std::fs::remove_dir_all(&old);

    status(qt, "Starting the service…", -1.0);
    std::fs::create_dir_all(unit_file.parent().unwrap())?;
    std::fs::write(
        &unit_file,
        format!(
            "# Written by Attacca's Roon Bridge setup. Managed by Attacca;\n\
             # remove via the app or delete together with {base}.\n\
             [Unit]\n\
             Description=Roon Bridge (installed by Attacca)\n\
             \n\
             [Service]\n\
             Environment=\"ROON_DATAROOT={data}\"\n\
             Environment=\"ROON_ID_DIR={data}\"\n\
             ExecStart=\"{program}/start.sh\"\n\
             Restart=on-failure\n\
             \n\
             [Install]\n\
             WantedBy=default.target\n",
            base = unit_path_str(base)?,
            data = unit_path_str(&data)?,
            program = unit_path_str(&program)?,
        ),
    )?;
    systemctl(&["daemon-reload"])?;
    systemctl(&["enable", "--now", UNIT_NAME])?;

    status(qt, "Waiting for the audio server…", -1.0);
    for i in 0..60 {
        if service_is_active() && raat_answers() {
            return Ok(
                "Roon Bridge is running. Now enable this computer's audio device in \
                 Roon Settings → Audio (from Roon on your phone or another computer)."
                    .to_owned(),
            );
        }
        // A few seconds' grace for "activating"; after that, inactive means
        // the unit failed and waiting the full timeout only hides the error.
        if i > 6 && !service_is_active() {
            bail!("the service failed to start — check `journalctl --user -u {UNIT_NAME}`");
        }
        std::thread::sleep(Duration::from_millis(500));
    }
    bail!("the service started but its audio server never came up — check `journalctl --user -u {UNIT_NAME}`")
}

fn do_uninstall() -> anyhow::Result<String> {
    let unit_file = unit_path().context("no XDG config directory")?;
    if unit_file.exists() {
        let _ = systemctl(&["disable", "--now", UNIT_NAME]);
        std::fs::remove_file(&unit_file)?;
        let _ = systemctl(&["daemon-reload"]);
    }
    if let Some(base) = base_dir() {
        if base.exists() {
            std::fs::remove_dir_all(&base)
                .context("service stopped, but removing the install directory failed")?;
        }
    }
    let _ = std::fs::remove_file(attacca_core::bridge_devices_path());
    Ok("Roon Bridge removed. Its zone disappears from Roon shortly.".to_owned())
}

fn do_set_device_mode(qt: &QtThread, id: &str, mode: &str) -> anyhow::Result<String> {
    // The id comes back from our own devices list, but it is also a path
    // component — accept only the hex file stem RAATServer generates.
    if id.is_empty() || !id.chars().all(|c| c.is_ascii_hexdigit()) {
        bail!("bad device id");
    }
    let path = settings_dir()
        .context("no XDG data directory")?
        .join(format!("device_{id}.json"));
    let text = std::fs::read_to_string(&path).context("device settings not found")?;
    let current: serde_json::Value = serde_json::from_str(&text)?;

    // Originals live in our config, not next to RAATServer's files, so its
    // Settings directory only ever contains files it expects.
    let originals_path = attacca_core::bridge_devices_path();
    let mut originals: serde_json::Value = std::fs::read_to_string(&originals_path)
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_else(|| serde_json::json!({}));
    // Indexing a non-object Value panics; a hand-edited or truncated file
    // must degrade to "no originals recorded", not take the worker down.
    if !originals.is_object() {
        originals = serde_json::json!({});
    }

    let new_settings = match mode {
        "pipewire" => {
            if current["output"]["device"] == "plug:pipewire" {
                return Ok("Already in desktop-mix mode.".to_owned());
            }
            originals[id] = current.clone();
            std::fs::create_dir_all(originals_path.parent().unwrap())?;
            std::fs::write(&originals_path, originals.to_string())?;
            // No "volume" block: the pipewire PCM has no hardware mixer, and
            // omitting it makes Roon fall back to software (DSP) volume.
            serde_json::json!({
                "output": {
                    "type": "alsa",
                    "device": "plug:pipewire",
                    "name": current["output"]["name"],
                },
                "unique_id": current["unique_id"],
                "external_config": current["external_config"],
            })
        }
        "exclusive" => {
            let Some(original) = originals.get(id).cloned() else {
                bail!("no saved exclusive-mode settings for this device — disable and re-enable it in Roon Settings → Audio");
            };
            original
        }
        other => bail!("unknown mode: {other}"),
    };

    std::fs::write(&path, new_settings.to_string())?;
    status(qt, "Restarting Roon Bridge…", -1.0);
    systemctl(&["restart", UNIT_NAME])?;
    Ok("Output switched — the zone drops for a few seconds while Roon Bridge restarts.".to_owned())
}
