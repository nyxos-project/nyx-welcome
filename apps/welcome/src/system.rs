//! Work off the window's thread: the Nyx System Manager (over the system bus) and
//! Flathub installs. Each job reports lines of progress, then its result.

use futures_util::StreamExt;
use nyx_system_proto::{Change, SystemProxy};

pub enum Job {
    /// The System Manager's settings (JSON).
    Settings,
    /// Apply changes in one rebuild.
    Apply(Vec<Change>),
    /// Update NyxOS and its apps.
    Update,
    /// Install Flathub apps for the user.
    Flatpaks(Vec<String>),
}

pub enum Event {
    Line(String),
    Done(Result<String, String>),
}

/// Run `job` on its own thread.
pub fn run(job: Job) -> async_channel::Receiver<Event> {
    let (tx, rx) = async_channel::unbounded();
    std::thread::spawn(move || {
        let result = match job {
            Job::Flatpaks(apps) => {
                let tx = tx.clone();
                nyx_catalog::flatpak(&apps, true, move |l| {
                    let _ = tx.send_blocking(Event::Line(l.to_string()));
                })
                .map(|_| format!("Installed {} from Flathub", apps.len()))
                .map_err(|e| format!("{e:#}"))
            }
            job => match tokio::runtime::Builder::new_current_thread().enable_all().build() {
                Ok(rt) => rt.block_on(manager(job, tx.clone())),
                Err(e) => Err(e.to_string()),
            },
        };
        let _ = tx.send_blocking(Event::Done(result));
    });
    rx
}

async fn manager(job: Job, tx: async_channel::Sender<Event>) -> Result<String, String> {
    let conn = zbus::Connection::system().await.map_err(|e| format!("Can't reach the system bus: {e}"))?;
    let proxy = SystemProxy::new(&conn).await.map_err(|e| e.to_string())?;
    let mut lines = proxy.receive_progress().await.map_err(|e| e.to_string())?;
    let forward = tokio::spawn(async move {
        while let Some(signal) = lines.next().await {
            if let Ok(args) = signal.args() {
                let _ = tx.send(Event::Line(args.line().to_string())).await;
            }
        }
    });
    let result = match job {
        Job::Settings => proxy.settings().await,
        Job::Apply(changes) => match serde_json::to_string(&changes) {
            Ok(json) => proxy.apply(&json).await,
            Err(e) => return Err(e.to_string()),
        },
        Job::Update => proxy.update().await,
        Job::Flatpaks(_) => unreachable!(),
    };
    forward.abort();
    result.map_err(|e| match e {
        zbus::Error::MethodError(_, Some(text), _) => text,
        e => e.to_string(),
    })
}
