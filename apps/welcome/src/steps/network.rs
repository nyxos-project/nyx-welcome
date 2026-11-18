//! Connect to the internet: the Wi-Fi networks around, through NetworkManager (`nmcli`).
//! Choosing software, signing in to accounts and the update at the end need a connection;
//! a cable connects by itself.

use super::{group, heading};
use crate::window::Ui;
use gtk::{glib, prelude::*};
use nyx_ui::widgets as w;
use std::process::{Command, Stdio};
use std::cell::RefCell;
use std::rc::Rc;

/// What lists the networks again (rows call it after connecting).
type Refresh = Rc<RefCell<Option<Rc<dyn Fn()>>>>;

#[derive(Debug, Clone, PartialEq)]
pub struct Network {
    pub ssid: String,
    pub signal: u8,
    pub secured: bool,
    pub in_use: bool,
}

/// `nmcli -t` fields: `:` separates them, `\:` and `\\` are a literal colon and backslash.
fn fields(line: &str) -> Vec<String> {
    let mut out = vec![String::new()];
    let mut chars = line.chars();
    while let Some(c) = chars.next() {
        match c {
            '\\' => {
                if let Some(n) = chars.next() {
                    out.last_mut().unwrap().push(n);
                }
            }
            ':' => out.push(String::new()),
            c => out.last_mut().unwrap().push(c),
        }
    }
    out
}

/// Networks from `nmcli -t -f IN-USE,SSID,SIGNAL,SECURITY device wifi list`: one per name
/// (the strongest of its access points), hidden ones left out, strongest first.
pub fn parse(list: &str) -> Vec<Network> {
    let mut nets: Vec<Network> = Vec::new();
    for line in list.lines() {
        let f = fields(line);
        if f.len() < 4 || f[1].is_empty() {
            continue;
        }
        let n = Network {
            ssid: f[1].clone(),
            signal: f[2].parse().unwrap_or(0),
            secured: !f[3].is_empty() && f[3] != "--",
            in_use: f[0] == "*",
        };
        match nets.iter_mut().find(|m| m.ssid == n.ssid) {
            Some(m) => {
                m.in_use |= n.in_use;
                if n.signal > m.signal {
                    m.signal = n.signal;
                }
            }
            None => nets.push(n),
        }
    }
    nets.sort_by(|a, b| b.in_use.cmp(&a.in_use).then(b.signal.cmp(&a.signal)));
    nets
}

fn output(args: &[&str]) -> String {
    Command::new("nmcli")
        .args(args)
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).into_owned())
        .unwrap_or_default()
}

/// Whether this computer is online, and through what: (connected, the connection's name).
fn status() -> (bool, String) {
    let state = output(&["-t", "-f", "STATE", "general"]);
    let active = output(&["-t", "-f", "NAME,TYPE", "connection", "show", "--active"]);
    let name = active
        .lines()
        .map(fields)
        .find(|f| f.len() > 1 && f[1] != "loopback" && f[1] != "bridge")
        .map(|f| f[0].clone())
        .unwrap_or_default();
    (state.trim().starts_with("connected"), name)
}

fn scan() -> Vec<Network> {
    parse(&output(&["-t", "-f", "IN-USE,SSID,SIGNAL,SECURITY", "device", "wifi", "list", "--rescan", "yes"]))
}

/// Connects; the password goes on standard input (`--ask`), never on a command line.
fn connect(ssid: &str, password: Option<&str>) -> Result<(), String> {
    let mut cmd = Command::new("nmcli");
    if password.is_some() {
        cmd.arg("--ask");
    }
    cmd.args(["device", "wifi", "connect", ssid]).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = cmd.spawn().map_err(|e| format!("nmcli didn't run: {e}"))?;
    if let (Some(pw), Some(mut stdin)) = (password, child.stdin.take()) {
        use std::io::Write;
        let _ = writeln!(stdin, "{pw}");
    }
    let out = child.wait_with_output().map_err(|e| e.to_string())?;
    if out.status.success() {
        Ok(())
    } else {
        let err = String::from_utf8_lossy(&out.stderr);
        let why = err.lines().last().unwrap_or("").trim_start_matches("Error:").trim().to_string();
        Err(if why.contains("Secrets were required") || why.contains("password") {
            "The password wasn't right.".into()
        } else if why.is_empty() {
            "Couldn't connect.".into()
        } else {
            why
        })
    }
}

/// Runs `work` off the main thread, then `done` with its result on it.
fn background<T: Send + 'static>(work: impl FnOnce() -> T + Send + 'static, done: impl FnOnce(T) + 'static) {
    let (tx, rx) = async_channel::bounded(1);
    std::thread::spawn(move || {
        let _ = tx.send_blocking(work());
    });
    glib::spawn_future_local(async move {
        if let Ok(v) = rx.recv().await {
            done(v);
        }
    });
}

pub fn page(_ui: &Rc<Ui>) -> gtk::Widget {
    let page = w::vbox(18);
    page.append(&heading(
        "Connect to the internet",
        "Installing software, signing in to accounts and updating need a connection. A cable connects by itself.",
    ));
    let state = w::label("Checking the connection…", "welcome-lede");
    state.set_xalign(0.0);
    page.append(&state);
    let networks = group("Wi-Fi networks");
    page.append(&networks.root);
    let list = w::vbox(0);
    networks.append(&list);
    let rescan = gtk::Button::with_label("Look again");
    rescan.set_halign(gtk::Align::Start);
    page.append(&rescan);

    let cell: Refresh = Rc::new(RefCell::new(None));
    let refresh: Rc<dyn Fn()> = {
        let (state, list, cell) = (state.clone(), list.clone(), cell.clone());
        Rc::new(move || {
            let (state, list, cell) = (state.clone(), list.clone(), cell.clone());
            background(
                || (status(), scan()),
                move |((online, name), nets)| {
                    state.set_text(&if online {
                        format!("Connected{}.", if name.is_empty() { String::new() } else { format!(" to {name}") })
                    } else {
                        "Not connected.".into()
                    });
                    while let Some(c) = list.first_child() {
                        list.remove(&c);
                    }
                    if nets.is_empty() {
                        list.append(&w::row("No Wi-Fi networks found", "Look again in a moment, or use a cable.", None));
                    }
                    for n in nets {
                        list.append(&network_row(&n, &state, &cell));
                    }
                },
            );
        })
    };
    *cell.borrow_mut() = Some(refresh.clone());
    let again = refresh.clone();
    rescan.connect_clicked(move |_| again());
    refresh();
    page.upcast()
}

fn network_row(n: &Network, state: &gtk::Label, refresh: &Refresh) -> gtk::Widget {
    let desc = format!(
        "{} · {}% signal",
        if n.in_use { "Connected" } else if n.secured { "Secured" } else { "Open" },
        n.signal
    );
    let row = w::vbox(6);
    let password = gtk::PasswordEntry::builder().show_peek_icon(true).placeholder_text("Password").build();
    password.set_visible(false);
    let connect_button = gtk::Button::with_label(if n.in_use { "Connected" } else { "Connect" });
    connect_button.set_sensitive(!n.in_use);
    row.append(&w::row(&n.ssid, &desc, Some(connect_button.upcast_ref())));
    row.append(&password);
    let (ssid, secured) = (n.ssid.clone(), n.secured);
    let (state, refresh, pw) = (state.clone(), refresh.clone(), password.clone());
    let go = Rc::new(move |b: &gtk::Button| {
        if secured && !pw.is_visible() {
            pw.set_visible(true);
            pw.grab_focus();
            return;
        }
        let secret = secured.then(|| pw.text().to_string());
        b.set_sensitive(false);
        b.set_label("Connecting…");
        let (ssid, state, refresh, b) = (ssid.clone(), state.clone(), refresh.clone(), b.clone());
        background(
            move || connect(&ssid, secret.as_deref()),
            move |result| match result {
                Ok(()) => {
                    let again = refresh.borrow().clone();
                    if let Some(again) = again {
                        again();
                    }
                }
                Err(why) => {
                    state.set_text(&why);
                    b.set_sensitive(true);
                    b.set_label("Connect");
                }
            },
        );
    });
    let g = go.clone();
    connect_button.connect_clicked(move |b| g(b));
    let b = connect_button.clone();
    password.connect_activate(move |_| go(&b));
    row.upcast()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn networks_are_one_per_name_strongest_first_with_the_connected_one_on_top() {
        let list = "\
:Cafe:40:WPA2
*:Home:60:WPA2 WPA3
:Home:80:WPA2 WPA3
:Open net:90:
::70:WPA2
:Escaped\\:name:20:--
";
        let n = parse(list);
        assert_eq!(n.iter().map(|n| n.ssid.as_str()).collect::<Vec<_>>(), ["Home", "Open net", "Cafe", "Escaped:name"]);
        assert!(n[0].in_use && n[0].signal == 80 && n[0].secured);
        assert!(!n[1].secured && !n[3].secured);
    }
}
