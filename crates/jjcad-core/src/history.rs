//! Undo/Redo mit Dokument-Snapshots.

use crate::model::Document;

pub struct History {
    undo: Vec<Document>,
    redo: Vec<Document>,
    limit: usize,
}

impl Default for History {
    fn default() -> Self {
        Self::new(80)
    }
}

impl History {
    pub fn new(limit: usize) -> Self {
        Self {
            undo: Vec::new(),
            redo: Vec::new(),
            limit: limit.max(4),
        }
    }

    /// Snapshot vor einer Änderung sichern.
    pub fn record(&mut self, doc: &Document) {
        let mut snap = doc.clone();
        snap.modified = false;
        snap.path = doc.path.clone();
        if self.undo.len() >= self.limit {
            self.undo.remove(0);
        }
        self.undo.push(snap);
        self.redo.clear();
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    /// Aktuelles Dokument zurückgeben und den Zustand als Redo ablegen.
    pub fn undo(&mut self, current: &Document) -> Option<Document> {
        let prev = self.undo.pop()?;
        let mut cur = current.clone();
        cur.modified = false;
        self.redo.push(cur);
        Some(prev)
    }

    pub fn redo(&mut self, current: &Document) -> Option<Document> {
        let next = self.redo.pop()?;
        let mut cur = current.clone();
        cur.modified = false;
        self.undo.push(cur);
        Some(next)
    }

    pub fn clear(&mut self) {
        self.undo.clear();
        self.redo.clear();
    }

    pub fn len(&self) -> usize {
        self.undo.len()
    }

    pub fn is_empty(&self) -> bool {
        self.undo.is_empty()
    }

    pub fn undo_label(&self) -> Option<String> {
        None
    }
}

/// Vergleich zweier Dokumente ohne UI-Zustand.
pub fn same_content(a: &Document, b: &Document) -> bool {
    let mut a = a.clone();
    let mut b = b.clone();
    a.modified = false;
    b.modified = false;
    a.path = None;
    b.path = None;
    // Der Bearbeitungsraum (Modell/Papierraum) ist Bedienzustand, kein Inhalt:
    // ein Wechsel darf keinen Undo-Schritt erzeugen.
    a.space = None;
    b.space = None;
    a == b
}
