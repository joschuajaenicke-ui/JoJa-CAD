//! # jjcad-core
//!
//! Kern von **JoJa-CAD** — 2D-CAD für Architektur und Bauantragszeichnungen.
//!
//! Der Kern ist GUI-frei und wird von der Desktop-Anwendung (`jjcad`) und dem
//! MCP-Server (`jjcad-mcp`) gemeinsam genutzt. Alle Funktionen laufen über die
//! Befehls-Schicht in [`actions`]; Dokumentmodell in [`model`], Anzeigeaufbau
//! in [`render`], Ein-/Ausgabe in [`persist`], [`dxf`], [`pdf`] und [`raster`].

pub mod actions;
pub mod area;
pub mod bild;
pub mod blocks_lib;
pub mod boundary;
pub mod daemmung;
pub mod daten;
pub mod dxf;
pub mod export;
pub mod flaeche;
pub mod fonts;
pub mod geom;
pub mod history;
pub mod i18n;
pub mod model;
pub mod pdf;
pub mod pdf_read;
pub mod persist;
pub mod raster;
pub mod render;
pub mod session;
pub mod snap;
pub mod stair;
pub mod style;
pub mod templates;
pub mod tessellate;
pub mod text;
pub mod transform;
pub mod wall;
pub mod units;
pub mod util;

pub use boundary::{detect_boundary, BoundaryError, BoundaryOpts, FillBoundary};
pub use geom::{v, Arc, BBox, Vec2, Vertex};
pub use model::{Document, Entity, EntityKind, Id, Layer, Sheet};
pub use session::Session;

pub const APP_NAME: &str = "JoJa-CAD";
pub const APP_VERSION: &str = env!("CARGO_PKG_VERSION");
pub const FILE_EXTENSION: &str = "jjcad";
