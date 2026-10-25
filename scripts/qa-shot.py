#!/usr/bin/env python3
"""QA-Hilfe: startet ein eigenes Xvfb + eine eigene JoJa-CAD-Kopie, macht einen
Screenshot und beendet beides wieder.

Wichtig: es wird ausschliesslich der selbst gestartete Prozess beendet (PID),
niemals per Namensmuster. Die Instanz des Nutzers bleibt unberuehrt.

Beispiel:
    python3 scripts/qa-shot.py --demo "arch,seldoor" --crop 300x420+1195+95 out.png
"""

import argparse
import os
import re
import shutil
import subprocess
import sys
import time

DISPLAY = ":99"


def start_xvfb(display: str) -> subprocess.Popen | None:
    if shutil.which("xdpyinfo") and subprocess.run(
        ["xdpyinfo"], env={**os.environ, "DISPLAY": display},
        capture_output=True,
    ).returncode == 0:
        return None
    p = subprocess.Popen(
        ["Xvfb", display, "-screen", "0", "1600x1000x24"],
        stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
    )
    time.sleep(1.5)
    return p


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("out")
    ap.add_argument("--file", default="samples/Wohnhaus.jjcad")
    ap.add_argument("--demo", default="")
    ap.add_argument("--crop", default="")
    ap.add_argument("--bin", default="target/release/jjcad")
    ap.add_argument("--wait", type=float, default=7.0)
    a = ap.parse_args()

    env = {k: v for k, v in os.environ.items() if k != "WAYLAND_DISPLAY"}
    env.update({"DISPLAY": DISPLAY, "LIBGL_ALWAYS_SOFTWARE": "1"})
    if a.demo:
        env["JJCAD_DEMO_UI"] = a.demo

    xvfb = start_xvfb(DISPLAY)
    app = subprocess.Popen(
        [os.path.abspath(a.bin), a.file],
        env=env, stdout=subprocess.DEVNULL, stderr=subprocess.PIPE,
    )
    try:
        time.sleep(a.wait)
        if app.poll() is not None:
            print("App beendet:", app.stderr.read().decode()[:400], file=sys.stderr)
            return 1
        info = subprocess.run(
            ["xwininfo", "-root", "-children"], env=env,
            capture_output=True, text=True,
        ).stdout
        m = re.search(r'(0x[0-9a-f]+) "(JoJa[^"]*)"', info)
        if not m:
            print("Kein Fenster gefunden", file=sys.stderr)
            return 1
        print("Fenster:", m.group(2))
        subprocess.run(["import", "-window", m.group(1), a.out], env=env)
        if a.crop:
            subprocess.run(["convert", a.out, "-crop", a.crop, "+repage", a.out])
        print("Screenshot:", a.out)
        return 0
    finally:
        app.terminate()
        try:
            app.wait(timeout=5)
        except subprocess.TimeoutExpired:
            app.kill()
        if xvfb is not None:
            xvfb.terminate()


if __name__ == "__main__":
    sys.exit(main())
