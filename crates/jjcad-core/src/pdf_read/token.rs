//! Inhalt eines PDF-Seitenstroms als Operanden und Operatoren.
//!
//! Der Parser selbst kommt von `lopdf` (bewährt, deckt auch kaputte Ströme
//! ab); diese Datei legt eine schmale, eigene Schnittstelle davor. So bleibt
//! der Rest des Lesers unabhängig von der Bibliothek — und die Zahlen sind
//! hier schon auf `f64` vereinheitlicht (`lopdf` mischt `i64` und `f32`).

use lopdf::{content::Content, Object};

/// Ein Operand eines PDF-Operators.
#[derive(Clone, Debug, PartialEq)]
pub enum Operand {
    Num(f64),
    Str(Vec<u8>),
    Name(String),
    Bool(bool),
    Array(Vec<Operand>),
    Dict,
    Null,
    Ref,
}

impl Operand {
    pub fn num(&self) -> Option<f64> {
        match self {
            Operand::Num(v) => Some(*v),
            Operand::Bool(b) => Some(if *b { 1.0 } else { 0.0 }),
            _ => None,
        }
    }
}

/// Ein Operator mit seinen Operanden (`"12 0 0 12 40 700 cm"` → `cm`).
#[derive(Clone, Debug)]
pub struct Op {
    pub op: String,
    pub args: Vec<Operand>,
}

/// Ergebnis einer Strom-Dekodierung.
pub struct Decoded {
    pub ops: Vec<Op>,
    /// Der Strom brach ab (kaputtes PDF) — der Leser zählt das als Warnung.
    pub truncated: bool,
}

fn convert(o: &Object) -> Operand {
    match o {
        Object::Null => Operand::Null,
        Object::Boolean(b) => Operand::Bool(*b),
        Object::Integer(i) => Operand::Num(*i as f64),
        Object::Real(r) => Operand::Num(*r as f64),
        Object::Name(n) => Operand::Name(String::from_utf8_lossy(n).into_owned()),
        Object::String(s, _) => Operand::Str(s.clone()),
        Object::Array(a) => Operand::Array(a.iter().map(convert).collect()),
        Object::Dictionary(_) => Operand::Dict,
        Object::Stream(_) => Operand::Dict,
        Object::Reference(_) => Operand::Ref,
    }
}

/// Dekodiert einen (bereits entpackten) Inhaltsstrom.
pub fn decode(data: &[u8]) -> Decoded {
    let mut ops = Vec::new();
    let mut truncated = false;
    for item in Content::decode_iter(data) {
        match item {
            Ok(op) => ops.push(Op {
                op: op.operator,
                args: op.operands.iter().map(convert).collect(),
            }),
            Err(_) => {
                truncated = true;
                break;
            }
        }
    }
    Decoded { ops, truncated }
}
