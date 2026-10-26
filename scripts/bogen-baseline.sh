#!/usr/bin/env bash
# Bogen-Baseline neu erzeugen: Messwerte, Screenshots, 4x-Ausschnitte.
#
# Aufruf:  ./scripts/bogen-baseline.sh [ZIELVERZEICHNIS]
#          (Vorgabe: docs/bogen-baseline — der Stand vor der Glättung;
#           für einen neuen Stand z. B. docs/bogen-baseline/nachher)
#
# Ergebnis (alles versioniert):
#   S1_klein.png … S4_starken.png    Vollbild je Szene (Build-Kennung im Fenstertitel)
#   S*_4x.png                        4x-Ausschnitt der Bogenmitte (Point-Sampling)
#   messwerte.md                     Messtabelle (Segmente, Bogenfehler, Knick, Zeit)
#   posen.txt                        Ansicht je Szene, wie sie die Screenshots nutzen
#
# Die Szenen-Zeichnungen erzeugt der Test selbst (samples/bogen_baseline/), sie
# sind ebenfalls versioniert. Es poppt nichts auf dem Bildschirm des Nutzers auf:
# qa-shot.py startet ein eigenes Xvfb und beendet nur den eigenen Prozess.
set -euo pipefail
cd "$(dirname "$0")/.."

OUT="${1:-docs/bogen-baseline}"
mkdir -p "$OUT"

echo "== 1/4 Zeichnungen und Ansichten =="
cargo test -q -p jjcad-core --test bogen_baseline -- --ignored --nocapture \
    | sed -n 's/^POSE //p' | tee "$OUT/posen.txt"

echo "== 2/4 Messwerte =="
cargo test -q -p jjcad-core --test bogen_baseline -- --nocapture \
    | sed -n '/^###/,$p' | sed '/^test /d' > "$OUT/messwerte.md"
tail -n 6 "$OUT/messwerte.md"

echo "== 3/4 Release-Build =="
cargo build --release -p jjcad-gui >/dev/null 2>&1

echo "== 4/4 Screenshots =="
while read -r name zoom cx cy; do
    # Ohne Demo-Zustand: `fit_view` passt die Szene ein (Fenster 1500x950),
    # damit der Screenshot ohne Klickweg reproduzierbar ist.
    python3 scripts/qa-shot.py \
        --file "samples/bogen_baseline/$name.jjcad" \
        --bin target/release/jjcad --wait 6 "$OUT/$name.png" >/dev/null
    echo "  $OUT/$name.png"
done < "$OUT/posen.txt"

# 4x-Ausschnitte: ein Weltpunkt je Szene (Bogenmitte) wird über Ansicht und
# Zeichenflächenmitte in Bildpixel umgerechnet. Ändert sich das Fensterlayout,
# sind die Ausschnitte nachzuziehen — die Vollbilder bleiben davon unberührt.
python3 - "$OUT" <<'PY'
import subprocess, sys
out = sys.argv[1]
# Zeichenflächenmitte im Fenster 1500x950 (aus dem Layout der Panels).
MITTE = (717.5, 507.5)
# Szene: (Weltpunkt in mm, Ausschnittbreite, Ausschnitthöhe)
ziele = {
    "S1_klein":   ((383.0, 133.0), 200, 150),
    "S2_mittel":  ((2200.0, 2400.0), 220, 160),
    "S3_gross":   ((0.0, 25000.0), 220, 160),
    "S4_starken": ((683.0, -167.0), 200, 160),
}
posen = {}
for zeile in open(f"{out}/posen.txt"):
    name, zoom, cx, cy = zeile.split()
    posen[name] = (float(zoom), float(cx), float(cy))
for name, ((wx, wy), w, h) in ziele.items():
    zoom, cx, cy = posen[name]
    sx = MITTE[0] + (wx - cx) * zoom
    sy = MITTE[1] - (wy - cy) * zoom
    x = int(sx - w / 2)
    y = int(sy - h / 2)
    subprocess.run(
        ["convert", f"{out}/{name}.png", "-crop", f"{w}x{h}+{x}+{y}", "+repage",
         "-filter", "point", "-resize", "400%", f"{out}/{name}_4x.png"],
        check=True,
    )
    print(f"  {out}/{name}_4x.png  ({w}x{h}+{x}+{y} bei 400 %)")
PY

echo "Fertig: $OUT/"
