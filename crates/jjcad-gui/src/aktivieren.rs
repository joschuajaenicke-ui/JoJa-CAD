//! Ein Fenster in den Vordergrund holen — der Weg unter Wayland (T0224).
//!
//! Unter Wayland kann kein Programm den Fokus an sich reißen: der Compositor
//! entscheidet. Der vorgesehene Weg heißt `xdg-activation`. Wer eine Datei aus
//! dem Dateibrowser öffnet, hat für seinen Klick ein **Token** erhalten — der
//! Browser gibt es dem gestarteten Programm als `XDG_ACTIVATION_TOKEN` mit.
//! Mit diesem Token darf ein Programm ein Fenster aktivieren:
//! `xdg_activation_v1.activate(token, surface)`.
//!
//! `winit`/`eframe` reichen das Token nicht weiter (`ViewportCommand::Focus`
//! ist unter Wayland wirkungslos), deshalb spricht dieses Modul das Protokoll
//! selbst — über **dieselbe** Verbindung, auf der das Fenster liegt: das
//! Fenster gehört zu dieser Verbindung, eine zweite könnte es nicht ansprechen
//! (`Backend::from_foreign_display`, der Weg, den `wayland-client` für
//! eingebettete Verbindungen anbietet).
//!
//! Nur unter Linux; auf allen anderen Systemen meldet [`aktiviere`] einen
//! Fehler, den der Aufrufer übergeht (dort wirkt `ViewportCommand::Focus`).

#![cfg(target_os = "linux")]

use raw_window_handle::{HasDisplayHandle, HasWindowHandle, RawDisplayHandle, RawWindowHandle};

use wayland_client::backend::ObjectId;
use wayland_client::globals::{registry_queue_init, GlobalListContents};
use wayland_client::protocol::wl_registry::WlRegistry;
use wayland_client::protocol::wl_surface::WlSurface;
use wayland_client::{Connection, Dispatch, Proxy, QueueHandle};
use wayland_protocols::xdg::activation::v1::client::xdg_activation_v1::XdgActivationV1;

/// Umgebungsvariable, in der ein gestartetes Programm das Token des Aufrufers
/// findet (xdg-activation; gesetzt von KDE/GNOME beim Start aus dem
/// Dateibrowser oder dem Startmenü).
pub const TOKEN_VARIABLE: &str = "XDG_ACTIVATION_TOKEN";

/// Leerer Zustand: die Liste der Globals füllt sich von selbst, dieses Modul
/// braucht kein Ereignis daraus.
struct Zustand;

impl Dispatch<WlRegistry, GlobalListContents> for Zustand {
    fn event(
        _state: &mut Self,
        _proxy: &WlRegistry,
        _event: <WlRegistry as Proxy>::Event,
        _data: &GlobalListContents,
        _conn: &Connection,
        _qhandle: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<XdgActivationV1, ()> for Zustand {
    fn event(
        _state: &mut Self,
        _proxy: &XdgActivationV1,
        _event: <XdgActivationV1 as Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        _qhandle: &QueueHandle<Self>,
    ) {
    }
}

/// Das Token aus der Umgebung des Prozesses, falls vorhanden.
pub fn token_aus_umgebung() -> Option<String> {
    let t = std::env::var(TOKEN_VARIABLE).ok()?;
    let t = t.trim();
    if t.is_empty() {
        None
    } else {
        Some(t.to_string())
    }
}

/// Verbindung und Oberfläche des Wayland-Fensters.
///
/// Die Verbindung ist **geliehen** (`from_foreign_display`): winit hat sie
/// aufgebaut und schließt sie, wenn das Fenster endet.
fn fester_anschluss(frame: &eframe::Frame) -> Result<(Connection, WlSurface), String> {
    let fenster = frame
        .window_handle()
        .map_err(|e| format!("kein Fenster: {e}"))?;
    let anzeige = frame
        .display_handle()
        .map_err(|e| format!("keine Anzeige: {e}"))?;
    match (fenster.as_raw(), anzeige.as_raw()) {
        (RawWindowHandle::Wayland(f), RawDisplayHandle::Wayland(a))
            if !f.surface.as_ptr().is_null() && !a.display.as_ptr().is_null() =>
        {
            // Sicher: die Zeiger gehören winit und leben, solange das Fenster
            // steht — dieser Aufruf läuft in dessen Zeichenweg.
            let backend = unsafe {
                wayland_client::backend::Backend::from_foreign_display(
                    a.display.as_ptr() as *mut _,
                )
            };
            let conn = Connection::from_backend(backend);
            let id = unsafe {
                ObjectId::from_ptr(WlSurface::interface(), f.surface.as_ptr() as *mut _)
            }
            .map_err(|e| format!("Oberfläche nicht ansprechbar: {e}"))?;
            let surface = WlSurface::from_id(&conn, id)
                .map_err(|e| format!("Oberfläche gehört nicht zur Verbindung: {e}"))?;
            Ok((conn, surface))
        }
        _ => Err("kein Wayland-Fenster".into()),
    }
}

/// Aktiviert das Fenster mit dem Token (Wayland).
///
/// `Err` sagt, warum es nicht ging — der Aufrufer übergeht das, weil der
/// Fokuswunsch daneben weiterläuft.
pub fn aktiviere(frame: &eframe::Frame, token: &str) -> Result<(), String> {
    let (conn, surface) = fester_anschluss(frame)?;
    // Die Globals einmal abfragen (ein Roundtrip; das Fenster zeichnet in
    // diesem Augenblick nicht, es steht in seinem eigenen Zeichenweg).
    let (globals, queue) = registry_queue_init::<Zustand>(&conn)
        .map_err(|e| format!("Globals nicht lesbar: {e}"))?;
    let qh = queue.handle();
    let activation: XdgActivationV1 = globals
        .bind::<XdgActivationV1, Zustand, ()>(&qh, 1..=1, ())
        .map_err(|e| format!("xdg_activation_v1 nicht nutzbar: {e}"))?;
    activation.activate(token.to_string(), &surface);
    conn.flush()
        .map_err(|e| format!("Senden fehlgeschlagen: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_nur_wenn_gesetzt_und_nicht_leer() {
        std::env::remove_var(TOKEN_VARIABLE);
        assert_eq!(token_aus_umgebung(), None);
        std::env::set_var(TOKEN_VARIABLE, "  ");
        assert_eq!(token_aus_umgebung(), None, "nur Leerzeichen ist kein Token");
        std::env::set_var(TOKEN_VARIABLE, " kwin-65 ");
        assert_eq!(token_aus_umgebung().as_deref(), Some("kwin-65"));
        std::env::remove_var(TOKEN_VARIABLE);
    }
}
