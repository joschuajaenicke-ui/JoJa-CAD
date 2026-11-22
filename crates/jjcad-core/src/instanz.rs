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
    Oeffnen {
        pfad: PathBuf,
        /// `xdg-activation`-Token des Aufrufers (T0224) — nur unter Wayland
        /// nötig, damit das Fenster wirklich nach vorn kommt.
        token: Option<String>,
    },
    /// Fenster nach vorn holen.
    Zeigen {
        /// Wie bei [`Auftrag::Oeffnen`].
        token: Option<String>,
    },
}

impl Auftrag {
    /// Zeile für den Kanal: `zeigen`, `oeffnen<TAB>pfad`, jeweils mit
    /// `<TAB>token` dahinter, wenn der Aufrufer eines mitbringt.
    ///
    /// Das Token steht **hinter dem letzten** Tabulatorzeichen — ein Pfad mit
    /// Tabulator darin ist damit nicht eindeutig, kommt aber in der Praxis
    /// nicht vor (und ein Token enthält selbst keinen).
    pub fn zeile(&self) -> String {
        fn mit_token(s: String, token: &Option<String>) -> String {
            match token {
                Some(t) if !t.is_empty() => format!("{s}\t{t}"),
                _ => s,
            }
        }
        match self {
            Auftrag::Zeigen { token } => mit_token("zeigen".to_string(), token),
            Auftrag::Oeffnen { pfad, token } => {
                mit_token(format!("oeffnen\t{}", pfad.display()), token)
            }
        }
    }

    /// Gegenstück zu [`Auftrag::zeile`]; `None` bei unbekannter Zeile.
    pub fn aus_zeile(zeile: &str) -> Option<Auftrag> {
        let zeile = zeile.trim_end_matches(['\r', '\n']);
        // Das Token steht hinter dem letzten Tabulatorzeichen. `zeigen` ohne
        // Tabulator hat keines.
        if zeile == "zeigen" {
            return Some(Auftrag::Zeigen { token: None });
        }
        if let Some(rest) = zeile.strip_prefix("zeigen\t") {
            let token = rest.trim();
            if token.is_empty() {
                return Some(Auftrag::Zeigen { token: None });
            }
            return Some(Auftrag::Zeigen {
                token: Some(token.to_string()),
            });
        }
        let rest = zeile.strip_prefix("oeffnen\t")?;
        if rest.is_empty() {
            return None;
        }
        // Pfad und optionales Token: Das Token reist nur mit, wenn es wie eines
        // aussieht (kein Leerzeichen, keine Pfadtrenner) — sonst gälte der Rest
        // als Pfad.
        let (pfad, token) = match rest.rsplit_once('\t') {
            Some((p, t)) => (p, Some(t.trim().to_string()).filter(|t| !t.is_empty())),
            None => (rest, None),
        };
        if pfad.is_empty() {
            return None;
        }
        Some(Auftrag::Oeffnen {
            pfad: PathBuf::from(pfad),
            token,
        })
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
        let a = Auftrag::Oeffnen {
            pfad: PathBuf::from("/tmp/Plan Nr. 2.jjcad"),
            token: None,
        };
        assert_eq!(Auftrag::aus_zeile(&a.zeile()), Some(a));
        assert_eq!(
            Auftrag::aus_zeile("zeigen"),
            Some(Auftrag::Zeigen { token: None })
        );
        assert_eq!(Auftrag::aus_zeile("quatsch"), None);
        assert_eq!(Auftrag::aus_zeile("oeffnen\t"), None, "leerer Pfad");
    }

    /// T0224: Das Aktivierungstoken reist mit — sonst könnte die laufende
    /// Instanz unter Wayland ihr Fenster nicht nach vorn holen.
    #[test]
    fn token_reist_mit() {
        let a = Auftrag::Oeffnen {
            pfad: PathBuf::from("/tmp/Haus.jjcad"),
            token: Some("kwin-65".into()),
        };
        assert_eq!(Auftrag::aus_zeile(&a.zeile()), Some(a));
        let z = Auftrag::Zeigen {
            token: Some("kwin-66".into()),
        };
        assert_eq!(Auftrag::aus_zeile(&z.zeile()), Some(z));
        // Ohne Token bleibt die Zeile wie bisher.
        assert_eq!(
            Auftrag::Zeigen { token: None }.zeile(),
            "zeigen",
            "ohne Token keine zusätzliche Spalte"
        );
    }

    #[test]
    fn ohne_laufende_instanz_wird_nichts_gemeldet() {
        let dir = test_dir("leer");
        assert!(!melden(&dir, &Auftrag::Zeigen { token: None }));
    }

    /// Die zweite Instanz meldet, die erste bekommt den Auftrag.
    #[test]
    fn zweite_instanz_uebergibt_an_die_erste() {
        let dir = test_dir("uebergabe");
        let dienst = dienst_starten(&dir, || {}).expect("Dienst");
        let auftrag = Auftrag::Oeffnen {
            pfad: PathBuf::from("/tmp/Haus.jjcad"),
            token: Some("kwin-7".into()),
        };
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
