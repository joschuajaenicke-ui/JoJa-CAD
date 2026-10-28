//! Farbeimer-Kern: die umgrenzende Fläche unter einem Klick ermitteln.
//!
//! Das Verfahren ist bewusst rasterbasiert. Schnittpunkte, Bögen, Gehrungen und
//! Wandkörper sind in einem Bilevel-Bild von selbst dicht; eine rein vektorielle
//! Flächensuche scheitert dagegen an jedem Ende, das nicht auf den Bruchteil
//! eines Millimeters genau passt.
//!
//! Ablauf in [`detect_boundary`]:
//! 1. Displayliste rendern — dieselbe Quelle wie Bildschirm, PDF und PNG.
//!    Zusätzlich beantwortet [`crate::snap::hit_test`], ob der Klick auf einer
//!    Linie oder in einem Wandkörper liegt (das ist eine Frage des Modells).
//! 2. Bilevel-Raster, 1 px = 0,5 mm. Passt die ganze Zeichnung nicht ins
//!    Pixelbudget, ermittelt ein grobes Raster erst Topologie und Ausdehnung
//!    der Fläche; das feine Raster deckt danach nur noch diese Fläche ab.
//! 3. Flutfüllung (4er-Nachbarschaft); Linien und Wandkörper sind Barrieren.
//!    Im feinen Raster wird nur der **Randstreifen** geflutet: das Innere der
//!    Fläche gilt über einen Kern aus dem groben Raster als bereits gefüllt.
//!    Dadurch kostet die Flut die Fläche nicht mehr, und 0,5 mm Auflösung
//!    bleibt auch für große Räume bezahlbar.
//! 4. Konturen der gefüllten Fläche ablaufen (Löcher getrennt).
//! 5. Douglas-Peucker im Rastergitter (Toleranz `tolerance_mm` zuzüglich eines
//!    halben Pixels Rasterrauschen), danach erst Umrechnung in Weltkoordinaten.
//!    Die Reihenfolge ist wichtig: der Pixelversatz an den Rasterkanten erzeugt
//!    ein Sägezahn, den Douglas-Peucker in Weltkoordinaten stehen ließe.
//!
//! Punkte (`Dot`) und Texte sind **keine** Begrenzung — ein Prüfpunkt mitten im
//! Raum darf die Fläche nicht zerteilen.
//!
//! Haarrisse — Linien, die sich in der Datei nicht auf den Bruchteil eines
//! Millimeters treffen — gelten bis [`SPALT_PX`] als geschlossen. Ohne diese
//! Toleranz liefe die Flut bei 0,5 mm durch jede Fuge zwischen Türblatt und
//! Wand und die Fläche wäre offen, obwohl der Plan geschlossen aussieht.

use crate::geom::*;
use crate::model::{Document, Id};
use crate::render::{render_document, DisplayList, RenderCtx, Shape};
use std::collections::{HashMap, HashSet};
use std::fmt;

// ---------------------------------------------------------------------------
// Schnittstelle
// ---------------------------------------------------------------------------

/// Optionen für [`detect_boundary`].
#[derive(Clone, Debug)]
pub struct BoundaryOpts {
    /// Toleranz der Polygon-Vereinfachung (Douglas-Peucker) in mm.
    pub tolerance_mm: f64,
    /// Obergrenze der Rasterpixel des feinen Rasters; begrenzt Speicher und
    /// Laufzeit. Reicht es nicht für 0,5 mm, wird gröber gerechnet.
    pub max_pixels: usize,
    /// Nur diese Layer als Begrenzung verwenden (`None` = alle sichtbaren).
    pub layer_filter: Option<Vec<Id>>,
}

impl Default for BoundaryOpts {
    fn default() -> Self {
        Self {
            tolerance_mm: 0.5,
            // 256 Mio. Zellen = 64 m² bei 0,5 mm. Das Feld wird vom Betriebssystem
            // nullinitialisiert geliehen und nur im Randstreifen berührt, kostet
            // also nur dort Speicher.
            max_pixels: 256_000_000,
            layer_filter: None,
        }
    }
}

/// Umgrenzung einer Fläche in Weltkoordinaten (mm).
///
/// `outer` läuft gegen den Uhrzeigersinn, jedes Loch in `holes` im Uhrzeigersinn.
/// Die Ringe sind geschlossen gedacht — der erste Punkt wird nicht wiederholt.
#[derive(Clone, Debug, PartialEq)]
pub struct FillBoundary {
    pub outer: Vec<Vec2>,
    pub holes: Vec<Vec<Vec2>>,
    /// Objekte, die die Fläche begrenzen — außen wie an den Inseln, in der
    /// Zeichenreihenfolge des Dokuments.
    ///
    /// Das ist der einzige Modellbezug dieser sonst rein geometrischen
    /// Auskunft. Ohne ihn wüsste die Füllfläche nicht, hinter welche Objekte
    /// sie in der Zeichenreihenfolge gehört: sie würde am Ende angehängt und
    /// ihre Fläche überdeckte die Linien ihrer eigenen Umgrenzung.
    pub border_entities: Vec<Id>,
}

impl FillBoundary {
    /// Fläche ohne Löcher in mm².
    pub fn area(&self) -> f64 {
        let aussen = polygon_area(&self.outer).abs();
        let loecher: f64 = self.holes.iter().map(|h| polygon_area(h).abs()).sum();
        (aussen - loecher).max(0.0)
    }

    /// Umfassendes Rechteck der Außenkontur.
    pub fn bbox(&self) -> BBox {
        BBox::from_points(&self.outer)
    }

    /// Liegt der Punkt in der Fläche (Löcher zählen als außen)?
    pub fn contains(&self, p: Vec2) -> bool {
        point_in_polygon(p, &self.outer) && !self.holes.iter().any(|h| point_in_polygon(p, h))
    }
}

/// Grund, warum keine Umgrenzung gefunden wurde.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BoundaryError {
    /// Fläche ist nach außen offen: die Flutfüllung hat den Zeichnungsrand
    /// erreicht (oder der Klick liegt außerhalb der Zeichnung).
    Open,
    /// Klick liegt auf einer Linie oder in einem Wandkörper.
    OnLine,
    /// Die Fläche ist für das Pixelbudget zu groß (Auflösung wäre unbrauchbar).
    TooLarge,
}

impl BoundaryError {
    /// Deutscher Text für Statuszeile/Toast.
    pub fn message(self) -> &'static str {
        match self {
            BoundaryError::Open => "Fläche ist nicht geschlossen",
            BoundaryError::OnLine => "auf einer Linie geklickt",
            BoundaryError::TooLarge => "Fläche ist zu groß zum Füllen",
        }
    }
}

impl fmt::Display for BoundaryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.message())
    }
}

impl std::error::Error for BoundaryError {}

// ---------------------------------------------------------------------------
// Raster
// ---------------------------------------------------------------------------

/// Zielauflösung: 1 px = 0,5 mm.
const ZIEL_PX_PRO_MM: f64 = 2.0;
/// Unter dieser Auflösung ist die Umgrenzung als Füllrand unbrauchbar.
const MIN_PX_PRO_MM: f64 = 0.05;
/// Rand um die Zeichnung, damit Randlinien vollständig im Raster liegen.
const RAND_MM: f64 = 1.0;
/// Kleinste Lochfläche in Rasterpixeln; darunter ist es ein Splitter.
const MIN_LOCH_PX2: f64 = 4.0;
/// Ab diesem Abstand (in groben Pixeln) zum Rand gilt eine Zelle als sicheres
/// Inneres. Zwei Ringe bleiben Randstreifen; der grobe Rand weicht höchstens
/// einen groben Pixel vom feinen ab.
const KERN_RAND: u8 = 3;
/// Spaltentoleranz in Pixeln, um die jede Linie verdickt wird.
///
/// Gezeichnete Linien treffen sich in der Datei nicht immer auf den Bruchteil
/// eines Millimeters genau — Türblatt und Bogen einer Tür lassen in diesem Plan
/// eine Lücke von etwa einem Millimeter. Bei 0,5 mm je Pixel wäre das ein Loch,
/// durch das die Flut in den Türbereich läuft; im groben Raster fiel es zu.
/// Ein Farbeimer überbrückt solche Haarrisse, sonst wäre er unbrauchbar.
const SPALT_PX: isize = 1;
/// Versatz der Kontur nach außen in Pixeln: ein halber Pixel für die
/// Linienmitte plus die Verdickung.
const VERSATZ_PX: f64 = SPALT_PX as f64 + 0.5;

const LEER: u8 = 0;
const BARRIERE: u8 = 1;
const GEFUELLT: u8 = 2;

/// Sicheres Inneres der Fläche aus dem groben Raster.
///
/// Ohne diese Maske müsste das feine Raster die ganze Fläche fluten — bei
/// 0,5 mm sind das für einen 27-m²-Raum 110 Mio. Zellen. Mit Maske genügt der
/// Randstreifen; die Laufzeit hängt dann am Umfang, nicht an der Fläche.
#[derive(Clone)]
struct Kern {
    /// Ursprung und Auflösung des groben Rasters.
    k: f64,
    x0: f64,
    y1: f64,
    /// Lage des Ausschnitts im groben Raster.
    bx0: usize,
    by0: usize,
    w: usize,
    h: usize,
    /// Abstand zur nächsten nicht gefüllten Zelle (4er-Nachbarschaft).
    dist: Vec<u8>,
}

impl Kern {
    /// Kern aus dem groben Raster über dem Ausschnitt `(x0, y0, x1, y1)`
    /// (jeweils einschließlich).
    fn neu(grob: &Gitter, bb: (usize, usize, usize, usize)) -> Self {
        let (bx0, by0, bx1, by1) = bb;
        let w = bx1 - bx0 + 1;
        let h = by1 - by0 + 1;
        let mut dist = vec![0u8; w * h];
        // Vorwärtspass der 4er-Distanztransformation, die Umrechnung des groben
        // Rasters gleich mitgenommen. Der Maskenrand zählt wie außen — die
        // Fläche berührt ihn ohnehin.
        for y in 0..h {
            for x in 0..w {
                let i = y * w + x;
                if grob.zellen[(by0 + y) * grob.w + bx0 + x] != GEFUELLT {
                    continue;
                }
                let mut m = u8::MAX;
                if x > 0 {
                    m = m.min(dist[i - 1]);
                }
                if y > 0 {
                    m = m.min(dist[i - w]);
                }
                if x == 0 || y == 0 {
                    m = 0;
                }
                dist[i] = m.saturating_add(1);
            }
        }
        for y in (0..h).rev() {
            for x in (0..w).rev() {
                let i = y * w + x;
                if dist[i] == 0 {
                    continue;
                }
                let mut m = u8::MAX;
                if x + 1 < w {
                    m = m.min(dist[i + 1]);
                }
                if y + 1 < h {
                    m = m.min(dist[i + w]);
                }
                if x + 1 == w || y + 1 == h {
                    m = 0;
                }
                dist[i] = dist[i].min(m.saturating_add(1));
            }
        }
        Self {
            k: grob.k,
            x0: grob.x0,
            y1: grob.y1,
            bx0,
            by0,
            w,
            h,
            dist,
        }
    }
}

/// Bilevel-Raster über einem Weltrechteck.
///
/// Pixel `(x, y)` deckt die Weltfläche
/// `[x0 + x/k, x0 + (x+1)/k] × [y1 - (y+1)/k, y1 - y/k]` ab; Gitterpunkt `(i, j)`
/// liegt auf der linken/oberen Kante von Pixel `(i, j)`.
struct Gitter {
    w: usize,
    h: usize,
    /// Pixel pro Millimeter
    k: f64,
    x0: f64,
    y1: f64,
    zellen: Vec<u8>,
    /// Sicheres Inneres; `None` = der Innenraum wird wirklich geflutet.
    kern: Option<Kern>,
    /// Feinspalte → Spalte im Kern (`usize::MAX` = außerhalb).
    kern_spalte: Vec<usize>,
    /// Feinzeile → Zeile im Kern (`usize::MAX` = außerhalb).
    kern_zeile: Vec<usize>,
    /// Von der Flut gefüllte Läufe `(Zeile, von, bis)` — Grundlage der
    /// Konturen, damit kein voller Scan über das Raster nötig ist.
    laeufe: Vec<(usize, usize, usize)>,
    /// Zelle → Objekt, das sie als Barriere gezeichnet hat. Nur im feinen
    /// Raster gefüllt (`None` = keine Aufzeichnung); Grundlage der
    /// Zeichenreihenfolge der Füllfläche.
    ///
    /// Eine Karte je Barrierezelle statt eines zweiten Feldes über alle Zellen:
    /// belegt wird nur der Umfang der eingezeichneten Linien, nicht die Fläche.
    eigentuemer: Option<HashMap<usize, Id>>,
    /// Objekt, dessen Primitiv gerade gezeichnet wird.
    eigentuemer_jetzt: Id,
}

impl Gitter {
    fn neu(bb: BBox, k: f64) -> Self {
        let k = k.max(MIN_PX_PRO_MM);
        let w = ((bb.width() * k).ceil() as usize).max(3) + 1;
        let h = ((bb.height() * k).ceil() as usize).max(3) + 1;
        Self {
            w,
            h,
            k,
            x0: bb.min.x,
            y1: bb.max.y,
            zellen: vec![LEER; w * h],
            kern: None,
            kern_spalte: Vec::new(),
            kern_zeile: Vec::new(),
            laeufe: Vec::new(),
            eigentuemer: None,
            eigentuemer_jetzt: 0,
        }
    }

    /// Aufzeichnen, welches Objekt welche Barrierezelle gezeichnet hat.
    ///
    /// Kostet eine Karte proportional zur Linienlänge im Raster — deshalb nur
    /// dort einschalten, wo die Grenzobjekte gebraucht werden.
    fn merke_eigentuemer(&mut self) {
        self.eigentuemer = Some(HashMap::new());
    }

    /// Gitterpunkt → Weltkoordinate.
    fn welt(&self, i: f64, j: f64) -> Vec2 {
        v(self.x0 + i / self.k, self.y1 - j / self.k)
    }

    /// Ring aus Gitterkoordinaten nach Welt; jede Kante um [`VERSATZ_PX`]
    /// nach außen versetzt.
    ///
    /// Die Zellkanten der Flutfüllung liegen vor der gezeichneten Linie (die
    /// Linie selbst sitzt auf den Pixelmitten, verdickt um [`SPALT_PX`]). Ohne
    /// diesen Versatz wäre die ermittelte Fläche durchgehend zu klein — bei
    /// 1 px = 0,5 mm um bis zu 1,2 mm. Gerechnet wird der Gehrungspunkt der
    /// beiden anliegenden Kanten: gerade Strecken rücken senkrecht um
    /// [`VERSATZ_PX`], Ecken liegen auf dem Schnittpunkt der beiden versetzten
    /// Kanten.
    fn ring_welt(&self, ring: &[(f64, f64)]) -> Vec<Vec2> {
        let n = ring.len();
        let norm = |d: (f64, f64)| {
            let l = d.0.hypot(d.1).max(1e-9);
            (d.1 / l, -d.0 / l)
        };
        (0..n)
            .map(|k| {
                let prev = ring[(k + n - 1) % n];
                let cur = ring[k];
                let next = ring[(k + 1) % n];
                let n1 = norm((cur.0 - prev.0, cur.1 - prev.1));
                let n2 = norm((next.0 - cur.0, next.1 - cur.1));
                // Gehrung: der Faktor 1/(1 + n1·n2) hält gerade Strecken bei
                // genau VERSATZ_PX und spreizt nur echte Ecken.
                let s = 1.0 / (1.0 + n1.0 * n2.0 + n1.1 * n2.1).max(0.25);
                self.welt(
                    cur.0 + VERSATZ_PX * s * (n1.0 + n2.0),
                    cur.1 + VERSATZ_PX * s * (n1.1 + n2.1),
                )
            })
            .collect()
    }

    /// Weltkoordinate → Pixel.
    fn pixel(&self, p: Vec2) -> Option<(usize, usize)> {
        let fx = (p.x - self.x0) * self.k;
        let fy = (self.y1 - p.y) * self.k;
        if !fx.is_finite() || !fy.is_finite() || fx < 0.0 || fy < 0.0 {
            return None;
        }
        let (x, y) = (fx as usize, fy as usize);
        if x >= self.w || y >= self.h {
            return None;
        }
        Some((x, y))
    }

    fn zustand(&self, x: usize, y: usize) -> u8 {
        self.zellen[y * self.w + x]
    }

    /// Kern setzen und die Zuordnung Feinraster → Kern vorbereiten.
    ///
    /// Ohne Tabellen bräuchte jede Zelle zwei Divisionen; bei einigen hundert-
    /// tausend Zellen im Randstreifen ist das der teuerste Teil der Flutung.
    fn setze_kern(&mut self, kern: Kern) {
        self.kern_spalte = (0..self.w)
            .map(|i| {
                let wx = self.x0 + (i as f64 + 0.5) / self.k;
                let s = (wx - kern.x0) * kern.k;
                if s < 0.0 {
                    return usize::MAX;
                }
                let s = s as usize;
                if s < kern.bx0 || s - kern.bx0 >= kern.w {
                    usize::MAX
                } else {
                    s - kern.bx0
                }
            })
            .collect();
        self.kern_zeile = (0..self.h)
            .map(|j| {
                let wy = self.y1 - (j as f64 + 0.5) / self.k;
                let z = (kern.y1 - wy) * kern.k;
                if z < 0.0 {
                    return usize::MAX;
                }
                let z = z as usize;
                if z < kern.by0 || z - kern.by0 >= kern.h {
                    usize::MAX
                } else {
                    z - kern.by0
                }
            })
            .collect();
        self.kern = Some(kern);
    }

    /// Liegt die Zelle im sicheren Inneren der Fläche?
    fn im_kern(&self, x: usize, y: usize) -> bool {
        if self.kern.is_none() {
            return false;
        }
        let (spalte, zeile) = (self.kern_spalte[x], self.kern_zeile[y]);
        if spalte == usize::MAX || zeile == usize::MAX {
            return false;
        }
        match &self.kern {
            Some(kern) => kern.dist[zeile * kern.w + spalte] >= KERN_RAND,
            None => false,
        }
    }

    /// Zelle, die geflutet werden darf: leer und nicht sicheres Inneres.
    fn frei(&self, x: i32, y: i32) -> bool {
        if x < 0 || y < 0 {
            return false;
        }
        let (x, y) = (x as usize, y as usize);
        if x >= self.w || y >= self.h {
            return false;
        }
        self.zellen[y * self.w + x] == LEER && !self.im_kern(x, y)
    }

    /// Gilt die Zelle als gefüllt (geflutet oder sicheres Inneres)?
    fn gefuellt(&self, x: i32, y: i32) -> bool {
        if x < 0 || y < 0 {
            return false;
        }
        let (x, y) = (x as usize, y as usize);
        if x >= self.w || y >= self.h {
            return false;
        }
        match self.zellen[y * self.w + x] {
            GEFUELLT => true,
            BARRIERE => false,
            _ => self.im_kern(x, y),
        }
    }

    fn barriere(&self, x: i32, y: i32) -> bool {
        if x < 0 || y < 0 {
            return false;
        }
        let (x, y) = (x as usize, y as usize);
        x < self.w && y < self.h && self.zellen[y * self.w + x] == BARRIERE
    }

    /// Eine Zelle als Barriere setzen.
    fn setze(&mut self, x: isize, y: isize) {
        if x < 0 || y < 0 || x as usize >= self.w || y as usize >= self.h {
            return;
        }
        let i = y as usize * self.w + x as usize;
        self.zellen[i] = BARRIERE;
        if self.eigentuemer.is_some() {
            let id = self.eigentuemer_jetzt;
            if let Some(map) = &mut self.eigentuemer {
                map.insert(i, id);
            }
        }
    }

    /// Objekte, deren Barriere an die gefüllte Fläche grenzt.
    ///
    /// Das sind die Objekte, die die Umgrenzung ziehen — außen wie an den
    /// Inseln. Ein Objekt weiter draußen (jenseits der nächsten Wand) berührt
    /// die Fläche nicht und bleibt außen vor.
    fn grenz_objekte(&self) -> Vec<Id> {
        let Some(map) = &self.eigentuemer else {
            return Vec::new();
        };
        let mut gesehen: HashSet<Id> = HashSet::new();
        let mut ids: Vec<Id> = Vec::new();
        for (&i, &id) in map {
            if gesehen.contains(&id) {
                continue;
            }
            let (x, y) = ((i % self.w) as i32, (i / self.w) as i32);
            let grenzt = self.gefuellt(x - 1, y)
                || self.gefuellt(x + 1, y)
                || self.gefuellt(x, y - 1)
                || self.gefuellt(x, y + 1);
            if grenzt {
                gesehen.insert(id);
                ids.push(id);
            }
        }
        ids
    }

    // -- Zeichnen ----------------------------------------------------------

    /// Alle Primitiven in das Raster zeichnen; `filter` begrenzt auf ein
    /// Weltrechteck (spart beim feinen Raster den Rest der Zeichnung).
    ///
    /// `schmuck` sind Objekte, die selbst Füllflächen sind: ihre Musterlinien
    /// und ihre Fläche sind Gestaltung, keine Begrenzung. Ohne diese Ausnahme
    /// würde ein bereits gefüllter Raum beim nächsten Klick in seine
    /// Schraffurstreifen zerfallen.
    fn zeichne(&mut self, dl: &DisplayList, filter: Option<&BBox>, schmuck: &HashSet<Id>) {
        for prim in &dl.prims {
            if schmuck.contains(&prim.entity) {
                continue;
            }
            if let Some(f) = filter {
                if !prim.bbox().intersects(f) {
                    continue;
                }
            }
            // Damit jede Barrierezelle ihrem Objekt zugeordnet werden kann.
            self.eigentuemer_jetzt = prim.entity;
            match &prim.shape {
                Shape::Line(a, b) => self.linie(*a, *b),
                Shape::Poly(pts, closed) => {
                    for i in 0..pts.len().saturating_sub(1) {
                        self.linie(pts[i], pts[i + 1]);
                    }
                    if *closed && pts.len() > 2 {
                        self.linie(pts[pts.len() - 1], pts[0]);
                    }
                }
                // Eine Füllfläche ist als Barriere gleichwertig mit ihrem
                // geschlossenen Rand — und der kostet nur den Umfang statt der
                // Fläche. Wandkörper mit 6 m² Fläche wären sonst Millionen
                // Schreibzugriffe je Klick.
                // Außenring *und* Lochränder sind Begrenzungen: eine bereits
                // gefüllte Fläche mit Insel darf die Flut nicht überrennen.
                Shape::Fill { outer, holes } => {
                    self.ring(outer);
                    for h in holes {
                        self.ring(h);
                    }
                }
                Shape::Circle { c, r } => {
                    let ring = self.kreis(*c, *r);
                    self.ring(&ring);
                }
                Shape::Arc { c, r, a0, a1 } => {
                    let pts = Arc::new(*c, *r, *a0, *a1).flatten(0.2 / self.k);
                    self.polyline(&pts);
                }
                Shape::Ellipse { c, rx, ry, rot } => {
                    let n = 72;
                    let ring: Vec<Vec2> = (0..n)
                        .map(|i| {
                            let t = i as f64 / n as f64 * std::f64::consts::TAU;
                            *c + v(rx * t.cos(), ry * t.sin()).rotated(*rot)
                        })
                        .collect();
                    self.ring(&ring);
                }
                // Punkt und Text sind Beschriftung, keine Begrenzung.
                Shape::Dot { .. } | Shape::Text(_) => {}
            }
        }
    }

    fn kreis(&self, c: Vec2, r: f64) -> Vec<Vec2> {
        // Bogenmaß so fein, dass die Sehne unter 0,25 px bleibt.
        let tol = 0.25 / self.k;
        let n = if r <= tol {
            8
        } else {
            let a = (1.0 - tol / r).clamp(-1.0, 1.0);
            (std::f64::consts::PI / a.acos()).ceil().clamp(16.0, 2048.0) as usize
        };
        (0..n)
            .map(|i| c + Vec2::polar(i as f64 / n as f64 * std::f64::consts::TAU, r))
            .collect()
    }

    fn ring(&mut self, pts: &[Vec2]) {
        self.polyline(pts);
        if pts.len() > 2 {
            self.linie(pts[pts.len() - 1], pts[0]);
        }
    }

    fn polyline(&mut self, pts: &[Vec2]) {
        for i in 0..pts.len().saturating_sub(1) {
            self.linie(pts[i], pts[i + 1]);
        }
    }

    /// Linie als Band aus drei 8er-Ketten (Bresenham) zeichnen.
    ///
    /// Eine 8er-Kette allein wäre für eine 4er-Flut schon undurchdringlich, aber
    /// Haarrisse an Stoßstellen blieben offen. Deshalb wird die Linie um
    /// [`SPALT_PX`] nach beiden Seiten versetzt und an den Enden verlängert.
    ///
    /// Bewusst drei versetzte Ketten statt eines quadratischen Aufdickens: die
    /// Außenkante des Bandes ist damit wieder eine Bresenham-Kette, also eine
    /// gerade Treppe. Ein 3×3-Block erzeugte an flachen Diagonalen einen
    /// Sägezahn, den Douglas-Peucker als echte Ecken behalten müsste.
    fn linie(&mut self, a: Vec2, b: Vec2) {
        let d = b - a;
        let l = d.len();
        let (a, b) = if l > 1e-9 && SPALT_PX > 0 {
            let e = d * (SPALT_PX as f64 / self.k / l);
            (a - e, b + e)
        } else {
            (a, b)
        };
        self.kette(a, b);
        if l > 1e-9 && SPALT_PX > 0 {
            let n = v(-d.y / l, d.x / l) * (SPALT_PX as f64 / self.k);
            self.kette(a + n, b + n);
            self.kette(a - n, b - n);
        }
    }

    /// Eine 8er-Kette (Bresenham).
    fn kette(&mut self, a: Vec2, b: Vec2) {
        let ax = (a.x - self.x0) * self.k;
        let ay = (self.y1 - a.y) * self.k;
        let bx = (b.x - self.x0) * self.k;
        let by = (self.y1 - b.y) * self.k;
        if !ax.is_finite() || !ay.is_finite() || !bx.is_finite() || !by.is_finite() {
            return;
        }
        // Zellen über `floor` zuordnen (nicht `round`): damit liegt die ideale
        // Linie in der markierten Zelle und der Abstand zur Flutkante ist
        // gleichmäßig einen halben Pixel — dieselbe Zuordnung wie bei `pixel()`,
        // sodass ein Klick auf der Linie sicher als `OnLine` erkannt wird.
        let mut x0 = ax.floor() as isize;
        let mut y0 = ay.floor() as isize;
        let x1 = bx.floor() as isize;
        let y1 = by.floor() as isize;
        let dx = (x1 - x0).abs();
        let dy = -(y1 - y0).abs();
        let sx = if x0 < x1 { 1 } else { -1 };
        let sy = if y0 < y1 { 1 } else { -1 };
        let mut err = dx + dy;
        loop {
            self.setze(x0, y0);
            if x0 == x1 && y0 == y1 {
                break;
            }
            let e2 = 2 * err;
            if e2 >= dy {
                err += dy;
                x0 += sx;
            }
            if e2 <= dx {
                err += dx;
                y0 += sy;
            }
        }
    }

    // -- Flutfüllung -------------------------------------------------------

    /// Saatzellen für das feine Raster: die feinen Zellen in den Randzellen des
    /// groben Rasters.
    ///
    /// Das grobe Raster hat *eine* zusammenhängende Fläche geflutet. Jeder
    /// Randstreifen dieser Fläche — außen wie an Löchern — berührt eine grobe
    /// Randzelle. Werden alle diese Zellen gesät, ist jeder Randstreifen des
    /// feinen Rasters erreichbar, auch ohne durch das (nicht geflutete) Innere
    /// zu laufen.
    fn saat(
        &self,
        grob: &Gitter,
        bb: (usize, usize, usize, usize),
        klick: (usize, usize),
    ) -> Vec<(usize, usize)> {
        let (bx0, by0, bx1, by1) = bb;
        let mut seeds = vec![klick];
        for cy in by0..=by1 {
            for cx in bx0..=bx1 {
                let i = cy * grob.w + cx;
                if grob.zellen[i] != GEFUELLT {
                    continue;
                }
                let am_rand = cx == 0
                    || cy == 0
                    || cx + 1 >= grob.w
                    || cy + 1 >= grob.h
                    || grob.zellen[i - 1] != GEFUELLT
                    || grob.zellen[i + 1] != GEFUELLT
                    || grob.zellen[i - grob.w] != GEFUELLT
                    || grob.zellen[i + grob.w] != GEFUELLT;
                if !am_rand {
                    continue;
                }
                // Nur ein kleines Feld um die Mitte der groben Randzelle säen.
                // Jede Randzelle einzeln zu säen kostet bei 0,5 mm Hunderttausende
                // Saatzellen; der Randstreifen ist aber zusammenhängend, ein
                // Saatfeld je Randzelle genügt.
                let mitte = grob.welt(cx as f64 + 0.5, cy as f64 + 0.5);
                let fx = ((mitte.x - self.x0) * self.k).floor() as isize;
                let fy = ((self.y1 - mitte.y) * self.k).floor() as isize;
                for dy in -1..=1 {
                    for dx in -1..=1 {
                        let (px, py) = (fx + dx, fy + dy);
                        if px >= 0 && py >= 0 && (px as usize) < self.w && (py as usize) < self.h {
                            if self.frei(px as i32, py as i32) {
                                seeds.push((px as usize, py as usize));
                            }
                        }
                    }
                }
            }
        }
        seeds
    }

    /// Flutfüllung (4er-Nachbarschaft) ab den Saatzellen.
    ///
    /// Liefert `(offen, bbox)`: `offen` heißt, die Fläche hat den Rasterrand
    /// erreicht. `bbox` ist der Pixelbereich der gefüllten Fläche
    /// (`x0, y0, x1, y1`, beide Grenzen einschließlich).
    fn flut(&mut self, seeds: &[(usize, usize)]) -> (bool, Option<(usize, usize, usize, usize)>) {
        let (w, h) = (self.w, self.h);
        let mut offen = false;
        let (mut min_x, mut min_y, mut max_x, mut max_y) = (usize::MAX, usize::MAX, 0, 0);
        let mut stapel: Vec<(usize, usize)> = seeds.to_vec();
        while let Some((x, y)) = stapel.pop() {
            if !self.frei(x as i32, y as i32) {
                continue;
            }
            // Ganzen freien Lauf dieser Zeile füllen (schneller als Pixel-BFS).
            let mut l = x;
            while l > 0 && self.frei(l as i32 - 1, y as i32) {
                l -= 1;
            }
            let mut r = x;
            while r + 1 < w && self.frei(r as i32 + 1, y as i32) {
                r += 1;
            }
            for i in l..=r {
                self.zellen[y * w + i] = GEFUELLT;
            }
            self.laeufe.push((y, l, r));
            min_x = min_x.min(l);
            max_x = max_x.max(r);
            min_y = min_y.min(y);
            max_y = max_y.max(y);
            if l == 0 || r == w - 1 || y == 0 || y == h - 1 {
                // Fläche läuft nach außen — weiteres Füllen ist sinnlos.
                offen = true;
                break;
            }
            for ny in [y.wrapping_sub(1), y + 1] {
                if ny >= h {
                    continue;
                }
                let mut i = l;
                while i <= r {
                    if self.frei(i as i32, ny as i32) {
                        stapel.push((i, ny));
                        while i <= r && self.frei(i as i32, ny as i32) {
                            i += 1;
                        }
                    } else {
                        i += 1;
                    }
                }
            }
        }
        if min_x == usize::MAX {
            (offen, None)
        } else {
            (offen, Some((min_x, min_y, max_x, max_y)))
        }
    }

    // -- Konturen ----------------------------------------------------------

    /// Rand der gefüllten Fläche als geschlossene Ringe in Gitterkoordinaten.
    ///
    /// Jede Kante läuft so, dass die gefüllte Fläche rechts von ihr liegt; damit
    /// hängen an jedem Gitterpunkt gleich viele ankommende wie abgehende Kanten
    /// und die Ringe lassen sich ohne Sonderfälle ablaufen.
    fn konturen(&self) -> Vec<Vec<(f64, f64)>> {
        // Nur die gefüllten Läufe ansehen. Ein Scan über das ganze Raster wäre
        // bei 0,5 mm unbezahlbar; das Innere der Fläche steht gar nicht im
        // Raster (es kommt aus dem Kern).
        let mut kanten: Vec<((i32, i32), (i32, i32))> = Vec::new();
        for (y, l, r) in &self.laeufe {
            let y = *y as i32;
            for x in *l as i32..=*r as i32 {
                if !self.gefuellt(x, y - 1) {
                    kanten.push(((x, y), (x + 1, y)));
                }
                if !self.gefuellt(x + 1, y) {
                    kanten.push(((x + 1, y), (x + 1, y + 1)));
                }
                if !self.gefuellt(x, y + 1) {
                    kanten.push(((x + 1, y + 1), (x, y + 1)));
                }
                if !self.gefuellt(x - 1, y) {
                    kanten.push(((x, y + 1), (x, y)));
                }
            }
        }
        let mut ausgehend: HashMap<(i32, i32), Vec<usize>> = HashMap::new();
        for (i, (s, _)) in kanten.iter().enumerate() {
            ausgehend.entry(*s).or_default().push(i);
        }
        let mut benutzt = vec![false; kanten.len()];
        let mut schleifen: Vec<Vec<(f64, f64)>> = Vec::new();
        for start in 0..kanten.len() {
            if benutzt[start] {
                continue;
            }
            let anfang = kanten[start].0;
            let mut ring: Vec<(f64, f64)> = Vec::new();
            let mut i = start;
            loop {
                benutzt[i] = true;
                let (a, b) = kanten[i];
                ring.push((a.0 as f64, a.1 as f64));
                if b == anfang {
                    break;
                }
                let dir = (b.0 - a.0, b.1 - a.1);
                let Some(kandidaten) = ausgehend.get(&b) else {
                    break;
                };
                let mut beste: Option<(usize, f64, f64)> = None;
                for cand in kandidaten {
                    if benutzt[*cand] {
                        continue;
                    }
                    let (c0, c1) = kanten[*cand];
                    let d = (c1.0 - c0.0, c1.1 - c0.1);
                    let kreuz = (dir.0 * d.1 - dir.1 * d.0) as f64;
                    let skalar = (dir.0 * d.0 + dir.1 * d.1) as f64;
                    let besser = match beste {
                        None => true,
                        // Rechtsturn bevorzugen: hält die Fläche geschlossen,
                        // wenn sich zwei Ecken nur diagonal berühren.
                        Some((_, bk, bs)) => {
                            kreuz > bk + 1e-9 || ((kreuz - bk).abs() <= 1e-9 && skalar > bs)
                        }
                    };
                    if besser {
                        beste = Some((*cand, kreuz, skalar));
                    }
                }
                match beste {
                    Some((next, _, _)) => i = next,
                    None => break,
                }
            }
            if ring.len() >= 4 {
                schleifen.push(ring);
            }
        }
        schleifen
    }
}

// ---------------------------------------------------------------------------
// Vereinfachung
// ---------------------------------------------------------------------------

/// Abstand Punkt–Gerade (unbegrenzt).
fn abstand_gerade(p: Vec2, a: Vec2, b: Vec2) -> f64 {
    let d = b - a;
    let l = d.len();
    if l < 1e-12 {
        return p.dist(a);
    }
    ((p - a).cross(d) / l).abs()
}

/// Douglas-Peucker auf einer offenen Kette; Anfang und Ende bleiben erhalten.
fn vereinfache_kette(pts: &[Vec2], tol: f64) -> Vec<Vec2> {
    let n = pts.len();
    if n <= 2 {
        return pts.to_vec();
    }
    let mut behalten = vec![false; n];
    behalten[0] = true;
    behalten[n - 1] = true;
    let mut stapel = vec![(0usize, n - 1)];
    while let Some((a, b)) = stapel.pop() {
        if b <= a + 1 {
            continue;
        }
        let (pa, pb) = (pts[a], pts[b]);
        let mut idx = a;
        let mut dmax = -1.0;
        for (i, p) in pts.iter().enumerate().take(b).skip(a + 1) {
            let d = abstand_gerade(*p, pa, pb);
            if d > dmax {
                dmax = d;
                idx = i;
            }
        }
        if dmax > tol {
            behalten[idx] = true;
            stapel.push((a, idx));
            stapel.push((idx, b));
        }
    }
    (0..n).filter(|i| behalten[*i]).map(|i| pts[i]).collect()
}

/// Douglas-Peucker auf einem geschlossenen Ring.
fn vereinfache_ring(pts: &[Vec2], tol: f64) -> Vec<Vec2> {
    let n = pts.len();
    if n <= 3 {
        return pts.to_vec();
    }
    // Zwei Anker: der Startpunkt allein würde die halbe Kontur an ihm hängen
    // lassen. Der am weitesten entfernte Punkt teilt den Ring in zwei Ketten.
    let mut fern = 0;
    let mut dmax = -1.0;
    for (i, p) in pts.iter().enumerate() {
        let d = p.dist2(pts[0]);
        if d > dmax {
            dmax = d;
            fern = i;
        }
    }
    if fern == 0 {
        return pts.to_vec();
    }
    let mut a = vereinfache_kette(&pts[..=fern], tol);
    let mut b: Vec<Vec2> = pts[fern..].to_vec();
    b.push(pts[0]);
    let mut b = vereinfache_kette(&b, tol);
    a.pop();
    b.pop();
    a.extend(b);
    a
}

/// Stützpunkte entfernen, die auf der Verbindung ihrer Nachbarn liegen.
///
/// Douglas-Peucker behält auf einem geschlossenen Ring beide Anker: den
/// Startpunkt und den am weitesten entfernten Punkt. Liegt der Startpunkt
/// mitten auf einer geraden Kante, bleibt dort ein überzähliger Punkt stehen —
/// ein Rechteck käme dann mit fünf statt vier Ecken heraus.
fn entferne_kollineare(pts: &mut Vec<Vec2>, eps: f64) {
    let mut aussortiert = true;
    while aussortiert && pts.len() > 3 {
        aussortiert = false;
        let mut i = 0;
        while i < pts.len() && pts.len() > 3 {
            let n = pts.len();
            let prev = pts[(i + n - 1) % n];
            let next = pts[(i + 1) % n];
            if abstand_gerade(pts[i], prev, next) <= eps {
                pts.remove(i);
                aussortiert = true;
            } else {
                i += 1;
            }
        }
    }
}

/// Aufeinanderfolgende Punkte, die enger als `eps` liegen, entfernen.
fn entdopple(pts: &mut Vec<Vec2>, eps: f64) {
    pts.dedup_by(|a, b| a.dist(*b) < eps);
    if pts.len() > 1 && pts[0].dist(pts[pts.len() - 1]) < eps {
        pts.pop();
    }
}

// ---------------------------------------------------------------------------
// Umgrenzung ermitteln
// ---------------------------------------------------------------------------

/// Umgrenzende Fläche unter `click` ermitteln.
///
/// `Ok` liefert Außenkontur und Löcher in Weltkoordinaten (mm). Fehler siehe
/// [`BoundaryError`].
pub fn detect_boundary(
    doc: &Document,
    click: Vec2,
    opts: &BoundaryOpts,
) -> Result<FillBoundary, BoundaryError> {
    let max_px = opts.max_pixels.max(10_000);
    let tol = opts.tolerance_mm.max(0.01);

    // Dieselbe Displayliste wie Bildschirm, PDF und PNG.
    let mut ctx = RenderCtx::new(doc);
    ctx.layer_filter = opts.layer_filter.clone();
    let dl = render_document(&ctx);
    if dl.is_empty() || !dl.bbox.is_valid() {
        return Err(BoundaryError::Open);
    }
    let zeichnung = dl.bbox.expanded(RAND_MM);
    // Füllflächen sind Gestaltung, keine Begrenzung (siehe `Gitter::zeichne`).
    let schmuck: HashSet<Id> = doc
        .entities
        .iter()
        // Volumen zählt wie eine Füllfläche: es ist Material und darf beim
        // Farbeimer weder übermalt noch als Umgrenzung gewählt werden.
        .filter(|e| {
            matches!(
                e.kind,
                crate::model::EntityKind::Fill { .. } | crate::model::EntityKind::Volume { .. }
            )
        })
        .map(|e| e.id)
        .collect();
    let flaeche_gesamt = (zeichnung.width() * zeichnung.height()).max(1e-6);

    // Klick auf einer Linie oder in einem Wandkörper? Das beantwortet das
    // Modell, nicht das Raster: `hit_test` kennt Wandkörper, Blöcke und die
    // Layer-Sichtbarkeit und trifft auch Stellen, an denen das Raster durch die
    // Spaltentoleranz schon eine Wand sieht.
    // Beschriftung zählt dabei nicht: ein Raumname über der Klickstelle ist keine
    // Linie, und `hit_test` trifft Text über sein Rechteck.
    if let Some(id) = crate::snap::hit_test(doc, click, tol.max(1.0), opts.layer_filter.as_deref()) {
        if let Some(e) = doc.entity(id) {
            if !matches!(
                e.kind,
                crate::model::EntityKind::Text { .. }
                    | crate::model::EntityKind::Point { .. }
                    | crate::model::EntityKind::Elevation { .. }
            ) {
                return Err(BoundaryError::OnLine);
            }
        }
    }

    // 1. Grobes Raster: Topologie und Ausdehnung der Fläche. Konturen werden
    //    hier nicht gebraucht — nur die gefüllte Fläche. Ein kleineres Budget
    //    kostet Genauigkeit nur im Randstreifen, den das feine Raster ohnehin
    //    neu rechnet.
    // Feste Obergrenze statt eines Anteils am Budget: das grobe Raster trägt
    // nur die Topologie und soll auch dann laufen, wenn das Budget für eine
    // feine Auflösung nicht reicht.
    let budget_grob = (max_px / 2).clamp(64_000, 2_000_000);
    let k_roh = (budget_grob as f64 / flaeche_gesamt).sqrt();
    if k_roh < MIN_PX_PRO_MM {
        return Err(BoundaryError::TooLarge);
    }
    let k_grob = k_roh.min(ZIEL_PX_PRO_MM);
    let mut grob = Gitter::neu(zeichnung, k_grob);
    grob.zeichne(&dl, None, &schmuck);
    let start = grob.pixel(click).ok_or(BoundaryError::Open)?;
    if grob.zustand(start.0, start.1) != LEER {
        return Err(BoundaryError::OnLine);
    }
    let (offen, bbox) = grob.flut(&[start]);
    if offen {
        return Err(BoundaryError::Open);
    }
    let Some(bb) = bbox else {
        return Err(BoundaryError::Open);
    };

    // 2. Feines Raster nur über der Fläche. Der Innenraum wird über den Kern
    //    als gefüllt vorausgesetzt; geflutet wird nur der Randstreifen.
    let kern = Kern::neu(&grob, bb);
    let region = BBox::new(
        grob.welt(bb.0 as f64, bb.3 as f64 + 1.0),
        grob.welt(bb.2 as f64 + 1.0, bb.1 as f64),
    );
    let roi = schnitt(&region.expanded(3.0 / grob.k + 2.0), &zeichnung);
    let flaeche_roi = (roi.width() * roi.height()).max(1e-6);
    let k_roh = (max_px as f64 / flaeche_roi).sqrt();
    if k_roh < MIN_PX_PRO_MM {
        return Err(BoundaryError::TooLarge);
    }
    let mut fein = Gitter::neu(roi, k_roh.min(ZIEL_PX_PRO_MM));
    fein.setze_kern(kern);
    // Das feine Raster ist die Grundlage der Grenzobjekte: es sieht den Rand
    // in der Auflösung, in der die Fläche entsteht.
    fein.merke_eigentuemer();
    fein.zeichne(&dl, Some(&roi.expanded(2.0)), &schmuck);
    let Some(s) = fein.pixel(click) else {
        return Err(BoundaryError::Open);
    };
    if fein.barriere(s.0 as i32, s.1 as i32) {
        return Err(BoundaryError::OnLine);
    }
    // 3. Fluten und auswerten. Erreicht die Flut den Rand der Region, war die
    //    Fläche nach außen offen — das feine Raster sieht Lücken, die im groben
    //    untergingen.
    let seeds = fein.saat(&grob, bb, s);
    let (offen, _) = fein.flut(&seeds);
    if offen {
        return Err(BoundaryError::Open);
    }
    // Grenzobjekte in der Zeichenreihenfolge des Dokuments: die Ordnung der
    // Karte ist die der Zellen, nicht die des Plans. Objekte aus Blocks stehen
    // nicht in `doc.entities` und fallen weg — sie lassen sich nicht ordnen.
    let mut grenz = fein.grenz_objekte();
    grenz.retain(|id| doc.entities.iter().any(|e| e.id == *id));
    grenz.sort_by_key(|id| doc.entities.iter().position(|e| e.id == *id).unwrap_or(usize::MAX));
    grenz.dedup();

    let mut ergebnis = aus_gitter(&fein, tol)?;
    ergebnis.border_entities = grenz;
    Ok(ergebnis)
}

/// Konturen des gefluteten Rasters als Umgrenzung ausgeben.
fn aus_gitter(gitter: &Gitter, tol: f64) -> Result<FillBoundary, BoundaryError> {
    let mut ringe: Vec<(f64, Vec<Vec2>)> = Vec::new();
    // Vereinfachung im Gitter, erst danach der Halbpixel-Versatz: im Gitter sind
    // die Treppen der Rasterung exakt, in Weltkoordinaten erzeugt der Versatz
    // ein Sägezahn von Toleranzgröße, den Douglas-Peucker stehen ließe.
    //
    // Zur Toleranz kommt ein halber Pixel: die Kante des Rasters selbst schwankt
    // um etwa einen Pixel (Verdickung, Bresenham-Treppe). Ohne diesen Zuschlag
    // läge die Toleranz genau auf dem Rauschen, und eine Wand käme mit
    // hunderten Stützpunkten heraus statt mit zwei.
    let tol_px = tol * gitter.k + 0.5;
    for ring in gitter.konturen() {
        if ring.len() < 4 {
            continue;
        }
        let roh = gitter.ring_welt(&ring);
        // Splitter (einzelne Pixel, z. B. Rest einer Schraffur) verwerfen.
        if polygon_area(&roh).abs() * gitter.k * gitter.k <= MIN_LOCH_PX2 {
            continue;
        }
        let als_punkte: Vec<Vec2> = ring.iter().map(|p| v(p.0, p.1)).collect();
        let vereinfacht = vereinfache_ring(&als_punkte, tol_px);
        let mut welt = gitter.ring_welt(
            &vereinfacht
                .iter()
                .map(|p| (p.x, p.y))
                .collect::<Vec<(f64, f64)>>(),
        );
        entdopple(&mut welt, 0.02);
        entferne_kollineare(&mut welt, 0.02);
        if welt.len() < 3 {
            continue;
        }
        ringe.push((polygon_area(&welt).abs(), welt));
    }
    if ringe.is_empty() {
        return Err(BoundaryError::Open);
    }
    // Die Außenkontur ist der Ring mit der größten Fläche — sie umschließt alle
    // Löcher.
    ringe.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
    let (_aussen_flaeche, mut outer) = ringe.remove(0);
    let mut holes: Vec<Vec<Vec2>> = ringe.into_iter().map(|(_, ring)| ring).collect();
    // Orientierung: außen gegen den Uhrzeigersinn, Löcher im Uhrzeigersinn.
    if polygon_area(&outer) < 0.0 {
        outer.reverse();
    }
    for loch in holes.iter_mut() {
        if polygon_area(loch) > 0.0 {
            loch.reverse();
        }
    }
    Ok(FillBoundary {
        outer,
        holes,
        border_entities: Vec::new(),
    })
}

/// Schnittmenge zweier Rechtecke (leer, wenn sie sich nicht überschneiden).
fn schnitt(a: &BBox, b: &BBox) -> BBox {
    let min = v(a.min.x.max(b.min.x), a.min.y.max(b.min.y));
    let max = v(a.max.x.min(b.max.x), a.max.y.min(b.max.y));
    if min.x >= max.x || min.y >= max.y {
        return *a;
    }
    BBox::new(min, max)
}
