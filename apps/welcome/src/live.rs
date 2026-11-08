//! What changes right away, for this user only (the theme, default apps), and what
//! Welcome reads from the system to show.

use std::process::Command;

/// Make `id` the system theme every Nyx app follows, and the lock screen's when
/// `lock` is set.
pub fn set_theme(id: &str, lock: bool) {
    if let Err(e) = nyx_theme::choose_system_theme(id) {
        eprintln!("nyx-welcome: {e}");
    }
    if lock {
        let _ = nyx_config::Layers::standard().set(
            nyx_config::Layer::User,
            "appearance",
            "lock-theme",
            toml::Value::String(id.into()),
        );
    }
}

pub fn set_density(density: &str) {
    nyx_theme::update_suite(|s| s.density = density.into());
    nyx_theme::apply();
}

/// Open a page of Nyx Settings.
pub fn open_settings(section: &str) {
    let _ = Command::new("nyx-settings").args(["--section", section]).spawn();
}

/// Start a program, if it's installed.
pub fn launch(program: &str) -> bool {
    Command::new(program).spawn().is_ok()
}

pub fn has_program(program: &str) -> bool {
    std::env::var_os("PATH").is_some_and(|path| std::env::split_paths(&path).any(|d| d.join(program).is_file()))
}

pub fn hostname() -> String {
    std::fs::read_to_string("/proc/sys/kernel/hostname").map(|s| s.trim().to_string()).unwrap_or_default()
}

fn output(program: &str, args: &[&str]) -> String {
    Command::new(program).args(args).output().map(|o| String::from_utf8_lossy(&o.stdout).into_owned()).unwrap_or_default()
}

pub fn time_zone() -> String {
    let tz = output("timedatectl", &["show", "-p", "Timezone", "--value"]);
    let tz = tz.trim();
    if tz.is_empty() { "UTC".into() } else { tz.into() }
}

pub fn time_zones() -> Vec<String> {
    let list: Vec<String> = output("timedatectl", &["list-timezones"]).lines().map(str::to_string).collect();
    if list.is_empty() { vec!["UTC".into()] } else { list }
}

/// The keyboard layouts offered, with their names.
pub const KEYBOARDS: &[(&str, &str)] = &[
    ("us", "English (US)"),
    ("gb", "English (UK)"),
    ("ca", "Canadian"),
    ("de", "German"),
    ("ch", "Swiss"),
    ("fr", "French"),
    ("be", "Belgian"),
    ("es", "Spanish"),
    ("latam", "Spanish (Latin America)"),
    ("pt", "Portuguese"),
    ("br", "Portuguese (Brazil)"),
    ("it", "Italian"),
    ("nl", "Dutch"),
    ("se", "Swedish"),
    ("no", "Norwegian"),
    ("dk", "Danish"),
    ("fi", "Finnish"),
    ("pl", "Polish"),
    ("cz", "Czech"),
    ("hu", "Hungarian"),
    ("ro", "Romanian"),
    ("tr", "Turkish"),
    ("gr", "Greek"),
    ("ru", "Russian"),
    ("ua", "Ukrainian"),
    ("il", "Hebrew"),
    ("ara", "Arabic"),
    ("in", "Indian"),
    ("jp", "Japanese"),
    ("kr", "Korean"),
    ("cn", "Chinese"),
];

/// The layout Hyprland has now.
pub fn keyboard() -> String {
    let out = output("hyprctl", &["-j", "getoption", "input:kb_layout"]);
    serde_json::from_str::<serde_json::Value>(&out)
        .ok()
        .and_then(|v| v["str"].as_str().map(str::to_string))
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "us".into())
}

/// The screens, as "DP-1: 2560×1440 at 144 Hz, scale 1.25".
pub fn screens() -> Vec<String> {
    let out = output("hyprctl", &["-j", "monitors"]);
    let Ok(serde_json::Value::Array(list)) = serde_json::from_str(&out) else { return Vec::new() };
    list.iter()
        .map(|m| {
            let name = m["description"].as_str().filter(|d| !d.is_empty()).or(m["name"].as_str()).unwrap_or("Screen");
            format!(
                "{name}: {}×{} at {:.0} Hz, scale {}",
                m["width"].as_i64().unwrap_or(0),
                m["height"].as_i64().unwrap_or(0),
                m["refreshRate"].as_f64().unwrap_or(0.0),
                m["scale"].as_f64().unwrap_or(1.0)
            )
        })
        .collect()
}

/// The person's full name (from their account), or their login.
pub fn real_name(user: &str) -> String {
    std::fs::read_to_string("/etc/passwd")
        .unwrap_or_default()
        .lines()
        .find_map(|l| {
            let f: Vec<&str> = l.split(':').collect();
            (f.first() == Some(&user)).then(|| f.get(4).and_then(|g| g.split(',').next()).unwrap_or("").to_string())
        })
        .filter(|n| !n.is_empty())
        .unwrap_or_else(|| user.to_string())
}

/// System generations: how many there are, and the current one's number.
pub fn generations() -> (usize, Option<u32>) {
    let mut numbers: Vec<u32> = std::fs::read_dir("/nix/var/nix/profiles")
        .map(|d| {
            d.flatten()
                .filter_map(|e| {
                    let n = e.file_name().to_string_lossy().into_owned();
                    n.strip_prefix("system-")?.strip_suffix("-link")?.parse().ok()
                })
                .collect()
        })
        .unwrap_or_default();
    numbers.sort();
    let current = std::fs::read_link("/nix/var/nix/profiles/system")
        .ok()
        .and_then(|l| l.to_string_lossy().strip_prefix("system-")?.strip_suffix("-link")?.parse().ok());
    (numbers.len(), current)
}
