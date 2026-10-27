//! Chat: Meldungen der Anwendung unten links in der Zeichenfläche.
//!
//! Die frühere Statuszeile, die den Nutzer zu einer Eingabe aufgefordert hat,
//! ist verschwunden; seitdem fehlt die Rückmeldung. Der Chat übernimmt sie:
//! geschlossen stehen die letzten Meldungen transparent unten links über der
//! Zeichnung und verblassen nach einigen Sekunden, `T` öffnet das vollständige
//! Protokoll (scrollbar, Zeichnung blass). Eingaben des Nutzers werden noch
//! nicht ausgewertet, sondern nur protokolliert.

use crate::app::App;
use crate::theme::Palette;
use egui::{Align, Align2, Color32, FontId, Frame, Layout, Margin, Painter, Pos2, Rect, RichText, Rounding, Stroke};

/// So lange bleibt eine Meldung im geschlossenen Zustand voll sichtbar (s).
pub const HALTEN_S: f64 = 3.0;
/// Danach verblasst sie bis zu diesem Alter vollständig (s).
pub const VERBLASSEN_S: f64 = 7.0;
/// Höchstzahl protokollierter Meldungen (ältere fallen weg).
pub const MAX_NACHRICHTEN: usize = 500;
/// Höchstzahl gleichzeitig sichtbarer Meldungen im geschlossenen Zustand.
pub const SICHTBAR: usize = 6;
/// Zeilenabstand der transparenten Meldungen (px).
const ZEILE_H: f32 = 19.0;

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
}

impl Chat {
    pub fn neu() -> Self {
        Self::default()
    }

    /// Meldung anhängen. Alte Meldungen über `MAX_NACHRICHTEN` fallen weg.
    pub fn sagen(&mut self, jetzt: f64, text: impl Into<String>) {
        let text = text.into();
        if text.trim().is_empty() {
            return;
        }
        self.nachrichten.push(Nachricht {
            text,
            zeit: jetzt,
            eingabe: false,
        });
        if self.nachrichten.len() > MAX_NACHRICHTEN {
            let weg = self.nachrichten.len() - MAX_NACHRICHTEN;
            self.nachrichten.drain(0..weg);
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
    pub fn eingabe_senden(&mut self, jetzt: f64) {
        let text = self.eingabe.trim().to_string();
        if text.is_empty() {
            return;
        }
        self.nachrichten.push(Nachricht {
            text,
            zeit: jetzt,
            eingabe: true,
        });
        if self.nachrichten.len() > MAX_NACHRICHTEN {
            let weg = self.nachrichten.len() - MAX_NACHRICHTEN;
            self.nachrichten.drain(0..weg);
        }
        self.eingabe.clear();
    }

    /// Öffnen und Schließen. Die Eingabezeile bekommt absichtlich keinen
    /// automatischen Fokus: sonst würde T beim nächsten Druck als Text im Feld
    /// landen statt das Protokoll wieder zu schließen.
    pub fn umschalten(&mut self) {
        self.offen = !self.offen;
    }

    pub fn schliessen(&mut self) {
        self.offen = false;
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

/// Meldungen unten links über der Zeichnung (geschlossener Chat).
pub fn zeichnen_geschlossen(chat: &Chat, p: &Painter, rect: Rect, jetzt: f64) {
    let sichtbare = chat.sichtbare(jetzt);
    let mut y = rect.bottom() - 26.0;
    for (n, a) in sichtbare.iter().rev() {
        let alpha = (255.0 * a) as u8;
        let farbe = if n.eingabe {
            Color32::from_rgba_unmultiplied(0x1F, 0x63, 0x9E, alpha)
        } else {
            Color32::from_rgba_unmultiplied(0x1A, 0x1D, 0x22, alpha)
        };
        p.text(
            Pos2::new(rect.left() + 18.0, y),
            Align2::LEFT_BOTTOM,
            &n.text,
            FontId::proportional(12.5),
            farbe,
        );
        y -= ZEILE_H;
    }
    // Dauerhafter, dezenter Hinweis auf das Protokoll
    p.text(
        Pos2::new(rect.left() + 18.0, rect.bottom() - 8.0),
        Align2::LEFT_BOTTOM,
        "T · Protokoll",
        FontId::proportional(10.0),
        Color32::from_rgba_unmultiplied(0x1A, 0x1D, 0x22, 70),
    );
}

/// Schleier über der Zeichnung, solange das Protokoll offen ist — die
/// Zeichnung bleibt sichtbar, tritt aber hinter den Text zurück.
pub fn schleier(rect: Rect) -> (Rect, Color32) {
    (
        rect,
        Color32::from_rgba_unmultiplied(
            Palette::PAPER.r(),
            Palette::PAPER.g(),
            Palette::PAPER.b(),
            200,
        ),
    )
}

/// Offenes Protokoll: alle Meldungen, scrollbar, mit Eingabezeile.
pub fn fenster(app: &mut App, ctx: &egui::Context) {
    if !app.chat.offen {
        return;
    }
    let flaeche = app.ui.last_canvas;
    if flaeche.width() < 120.0 || flaeche.height() < 120.0 {
        return;
    }
    let hoehe = (flaeche.height() * 0.52).clamp(160.0, 400.0);
    let breite = 520.0f32.min(flaeche.width() - 32.0);
    let pos = Pos2::new(flaeche.left() + 16.0, flaeche.bottom() - 16.0 - hoehe);

    egui::Area::new(egui::Id::new("chat_protokoll"))
        .order(egui::Order::Foreground)
        .fixed_pos(pos)
        .show(ctx, |ui| {
            ui.set_width(breite);
            Frame::none()
                .fill(Color32::from_rgba_unmultiplied(0x1A, 0x1D, 0x22, 238))
                .rounding(Rounding::same(10.0))
                .stroke(Stroke::new(1.0, Palette::BORDER))
                .inner_margin(Margin::symmetric(12.0, 10.0))
                .show(ui, |ui| {
                    ui.set_width(breite - 24.0);
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new("Protokoll")
                                .size(11.5)
                                .color(Palette::ACCENT),
                        );
                        ui.label(
                            RichText::new("T oder ESC schließt · Eingaben werden nur mitgeschrieben")
                                .size(10.0)
                                .color(Palette::TEXT_FAINT),
                        );
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if ui
                                .add(egui::Button::new(
                                    RichText::new("×").size(13.0).color(Palette::TEXT_DIM),
                                )
                                .fill(Color32::TRANSPARENT)
                                .stroke(Stroke::NONE))
                                .on_hover_text("Protokoll schließen")
                                .clicked()
                            {
                                app.chat.schliessen();
                            }
                        });
                    });
                    ui.add_space(4.0);
                    let liste_h = (hoehe - 84.0).max(60.0);
                    egui::ScrollArea::vertical()
                        .max_height(liste_h)
                        .auto_shrink([false, false])
                        .stick_to_bottom(true)
                        .show(ui, |ui| {
                            ui.set_width(breite - 44.0);
                            for n in &app.chat.nachrichten {
                                let text = if n.eingabe {
                                    RichText::new(format!("› {}", n.text))
                                        .size(12.0)
                                        .color(Palette::HOVER)
                                } else {
                                    RichText::new(n.text.as_str())
                                        .size(12.0)
                                        .color(Palette::TEXT)
                                };
                                ui.label(text);
                            }
                        });
                    ui.add_space(6.0);
                    let resp = ui.add(
                        egui::TextEdit::singleline(&mut app.chat.eingabe)
                            .hint_text("Nachricht … (wird nur protokolliert)")
                            .desired_width(f32::INFINITY),
                    );
                    let enter = ctx.input(|i| i.key_pressed(egui::Key::Enter));
                    if resp.lost_focus() && enter {
                        let jetzt = ctx.input(|i| i.time);
                        app.chat.eingabe_senden(jetzt);
                        resp.request_focus();
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
        chat.eingabe_senden(0.0);
        assert!(chat.nachrichten.is_empty());
        chat.eingabe = "Hallo".into();
        chat.eingabe_senden(1.0);
        assert_eq!(chat.nachrichten.len(), 1);
        assert!(chat.nachrichten[0].eingabe);
        assert!(chat.eingabe.is_empty());
    }

    /// Das Protokoll wächst nicht unbegrenzt.
    #[test]
    fn protokoll_ist_begrenzt() {
        let mut chat = Chat::neu();
        for i in 0..(MAX_NACHRICHTEN + 20) {
            chat.sagen(0.0, format!("{i}"));
        }
        assert_eq!(chat.nachrichten.len(), MAX_NACHRICHTEN);
        assert_eq!(chat.nachrichten.last().unwrap().text, format!("{}", MAX_NACHRICHTEN + 19));
    }

    /// T öffnet und schließt das Protokoll; offen fordert es Neuzeichnen an.
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
}
