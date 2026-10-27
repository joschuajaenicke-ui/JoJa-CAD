//! Chat: Meldungen der Anwendung unten links in der Zeichenfläche.
//!
//! Die frühere Statuszeile, die den Nutzer zu einer Eingabe aufgefordert hat,
//! ist verschwunden; seitdem fehlt die Rückmeldung. Der Chat übernimmt sie:
//! geschlossen stehen die letzten Meldungen transparent unten links über der
//! Zeichnung und verblassen nach einigen Sekunden. `^` (Taste unter ESC) öffnet das vollständige
//! Protokoll — im **selben** Bild wie die Ausgabe, ohne eigenes Fenster: die
//! Meldungen reichen weiter nach oben, unten steht die Eingabezeile, die wie
//! eine Meldung aussieht. Eingaben des Nutzers werden noch nicht ausgewertet,
//! sondern nur protokolliert.

use crate::app::App;
use crate::theme::Palette;
use egui::{Align2, Color32, FontId, Painter, Pos2, Rect, RichText, Stroke};

/// Taste zum Öffnen und Schließen des Protokolls: `^`, die Taste unter ESC.
pub const TASTE_OEFFNEN: egui::Key = egui::Key::Backtick;

/// So lange bleibt eine Meldung im geschlossenen Zustand voll sichtbar (s).
pub const HALTEN_S: f64 = 3.0;
/// Danach verblasst sie bis zu diesem Alter vollständig (s).
pub const VERBLASSEN_S: f64 = 7.0;
/// Höchstzahl protokollierter Meldungen (ältere fallen weg).
pub const MAX_NACHRICHTEN: usize = 500;
/// Höchstzahl gleichzeitig sichtbarer Meldungen im geschlossenen Zustand.
pub const SICHTBAR: usize = 6;
/// Zeilenabstand der Meldungen (px).
const ZEILE_H: f32 = 19.0;
/// Schriftgröße der Meldungen — für Ausgabe, Protokoll und Eingabe dieselbe.
const SCHRIFT: f32 = 12.5;
/// Linker Rand über der Zeichenfläche.
const RAND_X: f32 = 18.0;

/// Textfarbe der Anwendungsmeldungen (dunkel auf Papier).
fn text_farbe() -> Color32 {
    Color32::from_rgb(0x1A, 0x1D, 0x22)
}

/// Farbe der Nutzereingaben (abgesetzt, sonst gleiche Form).
fn eingabe_farbe() -> Color32 {
    Color32::from_rgb(0x1F, 0x63, 0x9E)
}

#[derive(Clone, Debug, PartialEq)]
pub struct Nachricht {
    pub text: String,
    /// Zeitpunkt der Meldung (Sekunden der egui-Uhr)
    pub zeit: f64,
    /// Eingabe des Nutzers (farblich abgesetzt, sonst nur protokolliert)
    pub eingabe: bool,
}

#[derive(Default)]
pub struct Chat {
    pub offen: bool,
    pub nachrichten: Vec<Nachricht>,
    pub eingabe: String,
    /// Die Eingabezeile soll beim nächsten Zeichnen den Fokus bekommen.
    pub fokus: bool,
    /// Wie viele Zeilen vom Ende zurückgeblättert ist (0 = neueste unten).
    pub scroll: usize,
}

impl Chat {
    pub fn neu() -> Self {
        Self::default()
    }

    /// Meldung anhängen. Alte Meldungen über `MAX_NACHRICHTEN` fallen weg.
    pub fn sagen(&mut self, jetzt: f64, text: impl Into<String>) {
        self.sagen_art(jetzt, text, false);
    }

    fn sagen_art(&mut self, jetzt: f64, text: impl Into<String>, eingabe: bool) {
        let text = text.into();
        if text.trim().is_empty() {
            return;
        }
        self.nachrichten.push(Nachricht {
            text,
            zeit: jetzt,
            eingabe,
        });
        if self.nachrichten.len() > MAX_NACHRICHTEN {
            let weg = self.nachrichten.len() - MAX_NACHRICHTEN;
            self.nachrichten.drain(0..weg);
            self.scroll = self.scroll.saturating_sub(weg);
        }
    }

    /// Wie [`Chat::sagen`], aber nur bei geändertem Text — für Aufforderungen,
    /// die jeden Frame anliegen (sonst stünde die Zeile 60-mal im Protokoll).
    pub fn sagen_wenn_neu(&mut self, jetzt: f64, text: &str) -> bool {
        if self.nachrichten.last().map(|n| n.text.as_str()) == Some(text) {
            return false;
        }
        self.sagen(jetzt, text);
        true
    }

    /// Eingabe des Nutzers protokollieren (wird noch nicht ausgewertet).
    /// Liefert `false`, wenn nichts zu senden war (leeres Feld).
    pub fn eingabe_senden(&mut self, jetzt: f64) -> bool {
        let text = self.eingabe.trim().to_string();
        if text.is_empty() {
            return false;
        }
        self.eingabe.clear();
        // Die eigene neue Zeile steht immer unten.
        self.scroll = 0;
        self.sagen_art(jetzt, text, true);
        true
    }

    /// Enter in der Eingabezeile: Text protokollieren und das Protokoll
    /// schließen — nach einer Eingabe gehört der Blick zurück auf die
    /// Zeichnung. Leeres Feld schließt nichts (es gab keine Eingabe).
    pub fn eingabe_abschicken(&mut self, jetzt: f64) -> bool {
        if !self.eingabe_senden(jetzt) {
            return false;
        }
        self.schliessen();
        true
    }

    /// Öffnen und Schließen. Beim Öffnen bekommt die Eingabezeile sofort den
    /// Fokus — man kann direkt losschreiben. Die Taste `^` schließt auch dann,
    /// wenn das Feld den Fokus hat (siehe `App::handle_shortcuts`).
    pub fn umschalten(&mut self) {
        self.offen = !self.offen;
        self.scroll = 0;
        self.fokus = self.offen;
    }

    pub fn schliessen(&mut self) {
        self.offen = false;
        self.fokus = false;
        self.eingabe.clear();
    }

    /// Im Protokoll blättern: `schritt > 0` weiter zurück (ältere Zeilen).
    pub fn blaettern(&mut self, schritt: i32, max_zeilen: usize) {
        let hoechstens = self.nachrichten.len().saturating_sub(max_zeilen.max(1));
        let neu = self.scroll as i64 + schritt as i64;
        self.scroll = neu.clamp(0, hoechstens as i64) as usize;
    }

    /// Die im geschlossenen Zustand sichtbaren Meldungen mit ihrer Deckkraft,
    /// älteste zuerst. Meldungen außerhalb des Zeitfensters entfallen.
    pub fn sichtbare(&self, jetzt: f64) -> Vec<(&Nachricht, f32)> {
        let mut out: Vec<(&Nachricht, f32)> = self
            .nachrichten
            .iter()
            .rev()
            .filter_map(|n| {
                let a = deckkraft(jetzt - n.zeit);
                (a > 0.01).then_some((n, a))
            })
            .take(SICHTBAR)
            .collect();
        out.reverse();
        out
    }

    /// Muss noch gezeichnet werden? (Verblassen läuft noch oder Chat offen)
    pub fn braucht_repaint(&self, jetzt: f64) -> bool {
        self.offen
            || self
                .nachrichten
                .iter()
                .rev()
                .take(SICHTBAR)
                .any(|n| jetzt - n.zeit < VERBLASSEN_S)
    }
}

/// Deckkraft einer Meldung nach ihrem Alter: voll bis `HALTEN_S`, linear auf
/// null bis `VERBLASSEN_S`, danach unsichtbar.
pub fn deckkraft(alter: f64) -> f32 {
    if alter <= HALTEN_S {
        1.0
    } else if alter >= VERBLASSEN_S {
        0.0
    } else {
        ((VERBLASSEN_S - alter) / (VERBLASSEN_S - HALTEN_S)) as f32
    }
}

/// Welche Meldungen das offene Protokoll zeigt: `(von, bis)` als Indexbereich,
/// `bis` ausschließlich. Neueste unten, `scroll` Zeilen weiter zurück.
pub fn protokoll_bereich(len: usize, scroll: usize, max_zeilen: usize) -> (usize, usize) {
    let max_zeilen = max_zeilen.max(1);
    let scroll = scroll.min(len.saturating_sub(max_zeilen));
    let bis = len - scroll;
    let von = bis.saturating_sub(max_zeilen);
    (von, bis)
}

/// Wie viele Zeilen passen ins Protokoll über der Eingabezeile?
pub fn zeilen_im_protokoll(rect: Rect) -> usize {
    (((rect.height() - 120.0) / ZEILE_H).floor() as i64).clamp(3, 200) as usize
}

/// Eine Meldungszeile mit Umbruch zeichnen; liefert die verbrauchte Höhe.
fn zeile(p: &Painter, x: f32, y_unten: f32, text: &str, farbe: Color32, breite: f32) -> f32 {
    let galley = p.layout(text.to_string(), FontId::proportional(SCHRIFT), farbe, breite);
    let h = galley.size().y;
    p.galley(Pos2::new(x, y_unten - h), galley, farbe);
    h
}

/// Eine Meldung samt ihrer Darstellung (Eingaben mit `›`, eigene Farbe).
fn darstellung(n: &Nachricht) -> (String, Color32) {
    if n.eingabe {
        (format!("› {}", n.text), eingabe_farbe())
    } else {
        (n.text.clone(), text_farbe())
    }
}

/// Meldungen unten links über der Zeichnung (geschlossener Chat).
pub fn zeichnen_geschlossen(chat: &Chat, p: &Painter, rect: Rect, jetzt: f64) {
    let breite = (rect.width() * 0.5).min(620.0);
    let mut y = rect.bottom() - 26.0;
    for (n, a) in chat.sichtbare(jetzt).iter().rev() {
        let alpha = (255.0 * a) as u8;
        let (text, c) = darstellung(n);
        let farbe = Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), alpha);
        let h = zeile(p, rect.left() + RAND_X, y, &text, farbe, breite);
        y -= h.max(ZEILE_H - 3.0) + 2.0;
    }
    // Dauerhafter, dezenter Hinweis auf das Protokoll
    p.text(
        Pos2::new(rect.left() + RAND_X, rect.bottom() - 8.0),
        Align2::LEFT_BOTTOM,
        "^ · Protokoll",
        FontId::proportional(10.0),
        Color32::from_rgba_unmultiplied(0x1A, 0x1D, 0x22, 70),
    );
}

/// Offenes Protokoll: dieselben Meldungen wie bei der Ausgabe, nur ohne
/// Verblassen und weiter nach oben; unten die Eingabezeile. Kein Fenster und
/// kein Rahmen — nur Text auf der blassen Zeichnung.
pub fn zeichnen_offen(chat: &Chat, p: &Painter, rect: Rect) {
    let breite = (rect.width() * 0.5).min(620.0);
    let max_zeilen = zeilen_im_protokoll(rect);
    let (von, bis) = protokoll_bereich(chat.nachrichten.len(), chat.scroll, max_zeilen);
    let mut y = rect.bottom() - 44.0;
    for n in chat.nachrichten[von..bis].iter().rev() {
        let (text, farbe) = darstellung(n);
        let h = zeile(p, rect.left() + RAND_X, y, &text, farbe, breite);
        y -= h.max(ZEILE_H - 3.0) + 2.0;
    }
    // Hinweiszeile: blättern, schließen, Eingaben werden nur mitgeschrieben
    // Keine Pfeil-Glyphen: die Systemschrift hat sie nicht (leere Kästen).
    let oben = if chat.scroll > 0 {
        format!("{} Zeilen zurück, Mausrad blättert", chat.scroll)
    } else {
        "Mausrad blättert".to_string()
    };
    p.text(
        Pos2::new(rect.left() + RAND_X, rect.bottom() - 8.0),
        Align2::LEFT_BOTTOM,
        format!("^ oder ESC schließt · {oben} · Enter schickt ab und schließt"),
        FontId::proportional(10.0),
        Color32::from_rgba_unmultiplied(0x1A, 0x1D, 0x22, 90),
    );
}

/// Schleier über der Zeichnung, solange das Protokoll offen ist — die
/// Zeichnung bleibt erkennbar, tritt aber hinter den Text zurück.
pub fn schleier(rect: Rect) -> (Rect, Color32) {
    (
        rect,
        Color32::from_rgba_unmultiplied(
            Palette::PAPER.r(),
            Palette::PAPER.g(),
            Palette::PAPER.b(),
            120,
        ),
    )
}

/// Eingabezeile des offenen Protokolls (rahmenloses Textfeld an genau der
/// Stelle, an der die nächste Meldung stünde) und das Blättern.
pub fn fenster(app: &mut App, ctx: &egui::Context) {
    if !app.chat.offen {
        return;
    }
    let flaeche = app.ui.last_canvas;
    if flaeche.width() < 200.0 || flaeche.height() < 160.0 {
        return;
    }

    // Blättern: Mausrad über der Zeichenfläche (dort nimmt die Zeichnung keine
    // Eingabe mehr an) oder Pfeiltasten.
    let max_zeilen = zeilen_im_protokoll(flaeche);
    let zeiger_drin = ctx
        .pointer_latest_pos()
        .map(|p| flaeche.contains(p))
        .unwrap_or(false);
    let rad = if zeiger_drin {
        ctx.input(|i| i.raw_scroll_delta.y)
    } else {
        0.0
    };
    let mut schritt = 0;
    if rad > 0.5 {
        schritt += 1;
    } else if rad < -0.5 {
        schritt -= 1;
    }
    if ctx.input(|i| i.key_pressed(egui::Key::ArrowUp) || i.key_pressed(egui::Key::PageUp)) {
        schritt += 3;
    }
    if ctx.input(|i| i.key_pressed(egui::Key::ArrowDown) || i.key_pressed(egui::Key::PageDown)) {
        schritt -= 3;
    }
    if schritt != 0 {
        app.chat.blaettern(schritt, max_zeilen);
    }

    // Breite ausdrücklich setzen: ohne feste Breite hat das rahmenlose Feld
    // keine anklickbare Fläche (die Area schrumpft auf ihren Inhalt).
    let breite = (flaeche.width() * 0.5).min(620.0);
    let pos = Pos2::new(flaeche.left() + RAND_X, flaeche.bottom() - 42.0);
    egui::Area::new(egui::Id::new("chat_eingabe"))
        .order(egui::Order::Foreground)
        .fixed_pos(pos)
        .show(ctx, |ui| {
            ui.set_min_width(breite);
            ui.set_max_width(breite);
            // Auf Papier: dunkler Text, dunkler Cursor, dezente Auswahl.
            ui.visuals_mut().text_cursor.stroke = Stroke::new(1.5, text_farbe());
            ui.visuals_mut().selection.bg_fill =
                Color32::from_rgba_unmultiplied(0x2F, 0x8C, 0xF0, 70);
            ui.horizontal(|ui| {
                ui.label(RichText::new("›").size(SCHRIFT).color(eingabe_farbe()));
                let resp = ui.add_sized(
                    [breite - 26.0, 18.0],
                    egui::TextEdit::singleline(&mut app.chat.eingabe)
                        .frame(false)
                        .font(FontId::proportional(SCHRIFT))
                        .text_color(text_farbe()),
                );
                if app.chat.fokus {
                    resp.request_focus();
                    app.chat.fokus = false;
                }
                let enter = ctx.input(|i| i.key_pressed(egui::Key::Enter));
                if resp.lost_focus() && enter {
                    let jetzt = ctx.input(|i| i.time);
                    if app.chat.eingabe_abschicken(jetzt) {
                        // Protokoll zu, Tastatur zurück an die Zeichenfläche.
                        ctx.memory_mut(|m| m.stop_text_input());
                    } else {
                        resp.request_focus();
                    }
                }
            });
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Voll sichtbar bis `HALTEN_S`, danach linear verblassend, dann weg.
    #[test]
    fn deckkraft_verlaeuft_stueckweise_linear() {
        assert_eq!(deckkraft(0.0), 1.0);
        assert_eq!(deckkraft(HALTEN_S), 1.0);
        let mitte = deckkraft((HALTEN_S + VERBLASSEN_S) / 2.0);
        assert!((mitte - 0.5).abs() < 1e-6, "Mitte: {mitte}");
        assert_eq!(deckkraft(VERBLASSEN_S), 0.0);
        assert_eq!(deckkraft(VERBLASSEN_S + 10.0), 0.0);
    }

    /// Geschlossen zeigt der Chat nur Meldungen innerhalb des Zeitfensters,
    /// höchstens `SICHTBAR`, neueste unten.
    #[test]
    fn sichtbare_meldungen_verblassen_und_sind_begrenzt() {
        let mut chat = Chat::neu();
        for i in 0..10 {
            chat.sagen(0.0, format!("Meldung {i}"));
        }
        let jetzt = 1.0;
        let s = chat.sichtbare(jetzt);
        assert_eq!(s.len(), SICHTBAR);
        assert_eq!(s.last().unwrap().0.text, "Meldung 9");
        // Nach dem Verblassen ist nichts mehr zu sehen.
        assert!(chat.sichtbare(VERBLASSEN_S + 1.0).is_empty());
        assert!(!chat.braucht_repaint(VERBLASSEN_S + 1.0));
        assert!(chat.braucht_repaint(1.0));
    }

    /// Eine Aufforderung, die jeden Frame anliegt, steht nur einmal im Protokoll.
    #[test]
    fn gleiche_meldung_wird_nicht_wiederholt() {
        let mut chat = Chat::neu();
        assert!(chat.sagen_wenn_neu(0.0, "Startpunkt klicken"));
        assert!(!chat.sagen_wenn_neu(0.5, "Startpunkt klicken"));
        assert!(chat.sagen_wenn_neu(1.0, "Nächster Punkt klicken – Enter beendet"));
        assert_eq!(chat.nachrichten.len(), 2);
    }

    /// Eingaben landen im Protokoll und leeren das Feld; leere Eingaben nicht.
    #[test]
    fn eingabe_wird_protokolliert() {
        let mut chat = Chat::neu();
        chat.eingabe = "   ".into();
        assert!(!chat.eingabe_senden(0.0));
        assert!(chat.nachrichten.is_empty());
        chat.eingabe = "Hallo".into();
        assert!(chat.eingabe_senden(1.0));
        assert_eq!(chat.nachrichten.len(), 1);
        assert!(chat.nachrichten[0].eingabe);
        assert!(chat.eingabe.is_empty());
    }

    /// Nach dem Abschicken ist das Protokoll wieder zu — die Meldung steht
    /// trotzdem im Chat und ist unten zu sehen. Ein leeres Feld schließt nichts.
    #[test]
    fn abschicken_schliesst_das_protokoll() {
        let mut chat = Chat::neu();
        chat.umschalten();
        assert!(chat.offen);
        // leeres Feld: kein Senden, bleibt offen
        assert!(!chat.eingabe_abschicken(1.0));
        assert!(chat.offen);
        // mit Text: protokolliert und geschlossen
        chat.eingabe = "Wandstärke 240".into();
        assert!(chat.eingabe_abschicken(2.0));
        assert!(!chat.offen);
        assert!(!chat.fokus);
        assert_eq!(chat.nachrichten.len(), 1);
        assert!(chat.nachrichten[0].eingabe);
        assert!(chat.eingabe.is_empty());
        // Die neue Zeile steht in der Ausgabe unten und ist sichtbar
        let sichtbar = chat.sichtbare(2.5);
        assert_eq!(sichtbar.len(), 1);
        assert_eq!(sichtbar[0].0.text, "Wandstärke 240");
        assert!(sichtbar[0].1 > 0.9);
    }

    /// Das Protokoll wächst nicht unbegrenzt.
    #[test]
    fn protokoll_ist_begrenzt() {
        let mut chat = Chat::neu();
        for i in 0..(MAX_NACHRICHTEN + 20) {
            chat.sagen(0.0, format!("{i}"));
        }
        assert_eq!(chat.nachrichten.len(), MAX_NACHRICHTEN);
        assert_eq!(
            chat.nachrichten.last().unwrap().text,
            format!("{}", MAX_NACHRICHTEN + 19)
        );
    }

    /// `^` öffnet und schließt das Protokoll; offen fordert es Neuzeichnen an.
    #[test]
    fn umschalten_oeffnet_und_schliesst() {
        let mut chat = Chat::neu();
        assert!(!chat.braucht_repaint(0.0));
        chat.umschalten();
        assert!(chat.offen);
        assert!(chat.braucht_repaint(0.0));
        chat.schliessen();
        assert!(!chat.offen);
    }

    /// Die Taste unter ESC ist es — nicht mehr T. Öffnen setzt den Fokus in die
    /// Eingabezeile, Schließen räumt eine halb getippte Eingabe weg.
    #[test]
    fn taste_ist_dach_und_oeffnen_fokussiert() {
        assert_eq!(TASTE_OEFFNEN, egui::Key::Backtick);
        let mut chat = Chat::neu();
        chat.umschalten();
        assert!(chat.offen && chat.fokus, "Eingabezeile muss Fokus bekommen");
        chat.eingabe = "halb".into();
        chat.schliessen();
        assert!(!chat.offen && !chat.fokus);
        assert!(chat.eingabe.is_empty());
    }

    /// Das offene Protokoll zeigt die neuesten Zeilen; Blättern verschiebt
    /// das Fenster und wird an beiden Rändern geklemmt.
    #[test]
    fn protokoll_blaettert_und_klemmt() {
        assert_eq!(protokoll_bereich(40, 0, 10), (30, 40));
        assert_eq!(protokoll_bereich(40, 5, 10), (25, 35));
        assert_eq!(protokoll_bereich(40, 99, 10), (0, 10));

        let mut chat = Chat::neu();
        for i in 0..40 {
            chat.sagen(0.0, format!("{i}"));
        }
        chat.blaettern(5, 10);
        assert_eq!(chat.scroll, 5);
        chat.blaettern(999, 10);
        assert_eq!(chat.scroll, 30, "höchstens bis zur ältesten Zeile");
        chat.blaettern(-999, 10);
        assert_eq!(chat.scroll, 0, "nicht unter die neueste Zeile");
        // eine neue eigene Meldung springt wieder ans Ende
        chat.blaettern(7, 10);
        chat.eingabe = "neu".into();
        chat.eingabe_senden(2.0);
        assert_eq!(chat.scroll, 0);
    }

    /// Weniger Meldungen als Platz: das Fenster bleibt am Ende.
    #[test]
    fn kurzes_protokoll_bleibt_am_ende() {
        assert_eq!(protokoll_bereich(3, 0, 10), (0, 3));
        assert_eq!(protokoll_bereich(0, 0, 10), (0, 0));
    }
}
