//! Eine Instanz je Datenwurzel (T0135).
//!
//! Eine zweite Instanz auf derselben Wurzel darf nicht schreiben: sie würde
//! `sitzung.json` überschreiben. Statt sie nur
//! abzuweisen, reicht sie ihre Arbeit an die laufende weiter — eine übergebene
//! Zeichnung wird dort als Reiter geöffnet, ohne Datei kommt das Fenster nach
//! vorn. Der zweite Prozess endet danach.
//!
//! Der Kanal ist ein TCP-Socket auf `127.0.0.1` mit zufälligem Port; Port und
//! Kennwort stehen in `<Wurzel>/instanz.port` (nur für den Eigentümer lesbar,
//! soweit das Dateisystem das hergibt). Damit gibt es **einen** Codeweg für
//! Linux, Windows und macOS. Ist kein Kanal erreichbar (alte Fassung,
//! abgestürzter Prozess, Rechte), meldet `melden` einfach `false` — der
//! Aufrufer fällt auf das bisherige Verhalten zurück.
//!
//! Der MCP-Server ist davon nicht betroffen: er nimmt keine Sperre und liest
//! weder Einstellungen noch Sitzung, läuft also unabhängig neben dem Programm.

use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver};
use std::time::Duration;

/// Datei in der Wurzel, die Port und Kennwort der laufenden Instanz trägt.
pub const KANAL_DATEI: &str = "instanz.port";

/// Was die laufende Instanz tun soll.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Auftrag {
    /// Zeichnung als Reiter öffnen (oder dorthin wechseln, wenn schon offen).
    Oeffnen(PathBuf),
    /// Fenster nach vorn holen.
    Zeigen,
}

impl Auftrag {
    /// Zeile für den Kanal: `zeigen` oder `oeffnen<TAB>pfad`.
    pub fn zeile(&self) -> String {
        match self {
            Auftrag::Zeigen => "zeigen".to_string(),
            Auftrag::Oeffnen(p) => format!("oeffnen\t{}", p.display()),
        }
    }

    /// Gegenstück zu [`Auftrag::zeile`]; `None` bei unbekannter Zeile.
    pub fn aus_zeile(zeile: &str) -> Option<Auftrag> {
        let zeile = zeile.trim_end_matches(['\r', '\n']);
        if zeile == "zeigen" {
            return Some(Auftrag::Zeigen);
        }
        zeile
            .strip_prefix("oeffnen\t")
            .filter(|p| !p.is_empty())
            .map(|p| Auftrag::Oeffnen(PathBuf::from(p)))
    }
}

fn kanal_pfad(wurzel: &Path) -> PathBuf {
    wurzel.join(KANAL_DATEI)
}

/// Meldet einen Auftrag an die laufende Instanz.
///
/// `true` = übergeben, der Aufrufer darf sich beenden. `false` = niemand hört
/// zu; dann gilt das bisherige Verhalten (Sperre nehmen oder Hinweis zeigen).
pub fn melden(wurzel: &Path, auftrag: &Auftrag) -> bool {
    let Ok(text) = std::fs::read_to_string(kanal_pfad(wurzel)) else {
        return false;
    };
    let mut teile = text.split_whitespace();
    let (Some(port), Some(kennwort)) = (teile.next(), teile.next()) else {
        return false;
    };
    let Ok(port) = port.parse::<u16>() else {
        return false;
    };
    let Ok(mut strom) = TcpStream::connect(("127.0.0.1", port)) else {
        return false;
    };
    let _ = strom.set_write_timeout(Some(Duration::from_millis(500)));
    writeln!(strom, "{kennwort}\t{}", auftrag.zeile()).is_ok()
}

/// Laufender Dienst der eigenen Instanz: nimmt Aufträge entgegen und weckt die
/// Oberfläche. Beim Ablegen wird die Kanaldatei entfernt, sofern sie noch die
/// eigene ist.
pub struct Dienst {
    rx: Receiver<Auftrag>,
    waechter: Waechter,
}

impl Dienst {
    /// Holt alle wartenden Aufträge ab (ohne zu blockieren).
    pub fn abholen(&self) -> Vec<Auftrag> {
        let mut out = Vec::new();
        while let Ok(a) = self.rx.try_recv() {
            out.push(a);
        }
        out
    }
}

struct Waechter {
    pfad: PathBuf,
    kennwort: String,
}

impl Drop for Waechter {
    fn drop(&mut self) {
        if let Ok(t) = std::fs::read_to_string(&self.pfad) {
            if t.contains(&self.kennwort) {
                let _ = std::fs::remove_file(&self.pfad);
            }
        }
    }
}

/// Startet den Dienst für diese Wurzel. `wecken` wird nach jedem empfangenen
/// Auftrag gerufen — die Oberfläche zeichnet sonst erst bei Eingabe neu.
///
/// `None`, wenn kein Port frei ist oder die Kanaldatei nicht geschrieben werden
/// kann; dann bleibt es beim bisherigen Verhalten.
pub fn dienst_starten(
    wurzel: &Path,
    wecken: impl Fn() + Send + 'static,
) -> Option<Dienst> {
    let listener = TcpListener::bind(("127.0.0.1", 0)).ok()?;
    let port = listener.local_addr().ok()?.port();
    let kennwort = crate::sicherung::neue_uuid();
    let pfad = kanal_pfad(wurzel);
    let _ = std::fs::write(&pfad, format!("{port} {kennwort}\n"));
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&pfad, std::fs::Permissions::from_mode(0o600));
    }

    let (tx, rx) = channel();
    let eigenes = kennwort.clone();
    std::thread::Builder::new()
        .name("jjcad-instanz".into())
        .spawn(move || {
            for strom in listener.incoming().flatten() {
                let mut zeile = String::new();
                if BufReader::new(&strom).read_line(&mut zeile).is_err() {
                    continue;
                }
                let Some((kennwort, rest)) = zeile.split_once('\t') else {
                    continue;
                };
                if kennwort != eigenes {
                    continue;
                }
                if let Some(a) = Auftrag::aus_zeile(rest) {
                    if tx.send(a).is_ok() {
                        wecken();
                    }
                }
            }
        })
        .ok()?;

    Some(Dienst {
        rx,
        waechter: Waechter { pfad, kennwort },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_dir(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!(
            "jjcad-instanz-{}-{name}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn auftrag_ueberlebt_die_zeile() {
        let a = Auftrag::Oeffnen(PathBuf::from("/tmp/Plan Nr. 2.jjcad"));
        assert_eq!(Auftrag::aus_zeile(&a.zeile()), Some(a));
        assert_eq!(Auftrag::aus_zeile("zeigen"), Some(Auftrag::Zeigen));
        assert_eq!(Auftrag::aus_zeile("quatsch"), None);
        assert_eq!(Auftrag::aus_zeile("oeffnen\t"), None, "leerer Pfad");
    }

    #[test]
    fn ohne_laufende_instanz_wird_nichts_gemeldet() {
        let dir = test_dir("leer");
        assert!(!melden(&dir, &Auftrag::Zeigen));
    }

    /// Die zweite Instanz meldet, die erste bekommt den Auftrag.
    #[test]
    fn zweite_instanz_uebergibt_an_die_erste() {
        let dir = test_dir("uebergabe");
        let dienst = dienst_starten(&dir, || {}).expect("Dienst");
        let auftrag = Auftrag::Oeffnen(PathBuf::from("/tmp/Haus.jjcad"));
        assert!(melden(&dir, &auftrag), "Meldung kam nicht an");
        // Der Empfang läuft im Thread — kurz warten.
        let mut empfangen = Vec::new();
        for _ in 0..50 {
            empfangen.extend(dienst.abholen());
            if !empfangen.is_empty() {
                break;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        assert_eq!(empfangen, vec![auftrag]);
    }

    /// Ein fremdes Kennwort wird ignoriert — der Kanal ist keine offene Tür.
    #[test]
    fn falsches_kennwort_wird_ignoriert() {
        let dir = test_dir("kennwort");
        let dienst = dienst_starten(&dir, || {}).expect("Dienst");
        let text = std::fs::read_to_string(kanal_pfad(&dir)).unwrap();
        let port: u16 = text.split_whitespace().next().unwrap().parse().unwrap();
        let mut strom = TcpStream::connect(("127.0.0.1", port)).expect("Port");
        writeln!(strom, "falsch\tzeigen").unwrap();
        drop(strom);
        std::thread::sleep(Duration::from_millis(100));
        assert!(dienst.abholen().is_empty(), "fremder Auftrag kam durch");
    }

    /// Beim Ablegen verschwindet die Kanaldatei — eine neue Instanz findet
    /// keinen toten Kanal vor.
    #[test]
    fn kanaldatei_verschwindet_beim_ablegen() {
        let dir = test_dir("aufraeumen");
        {
            let _dienst = dienst_starten(&dir, || {}).expect("Dienst");
            assert!(kanal_pfad(&dir).exists());
        }
        assert!(!kanal_pfad(&dir).exists(), "Kanaldatei blieb liegen");
    }
}
