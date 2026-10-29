#!/usr/bin/env python3
"""Klickprüfung: Treppe mit drei Punkten setzen (T0027).

Startet ein eigenes Xvfb + eine eigene JoJa-CAD-Instanz (die Instanz des
Nutzers bleibt unberührt): Werkzeug „Treppe" per Taste B, dann drei Klicks —
Lauflinie Anfang, Lauflinie Ende, Breite. Danach wird die Treppe angeklickt,
damit das Eigenschaften-Panel die Maße zeigt.

    python3 scripts/klick-treppe.py            # Debug-Binary
    BIN=target/release/jjcad python3 scripts/klick-treppe.py
"""

import ctypes
import ctypes.util
import os
import subprocess
import sys
import time

DISPLAY = ":98"
W, H = 1600, 1000
BIN = os.environ.get("BIN", "target/debug/jjcad")
DATEI = os.environ.get("FILE", "samples/Wohnhaus.jjcad")

x11 = ctypes.cdll.LoadLibrary(ctypes.util.find_library("X11"))
xtst = ctypes.cdll.LoadLibrary(ctypes.util.find_library("Xtst"))
x11.XOpenDisplay.restype = ctypes.c_void_p
x11.XOpenDisplay.argtypes = [ctypes.c_char_p]
DISPLAY_HANDLE = None
x11.XStringToKeysym.restype = ctypes.c_ulong
x11.XKeysymToKeycode.argtypes = [ctypes.c_void_p, ctypes.c_ulong]
x11.XSetInputFocus.argtypes = [ctypes.c_void_p, ctypes.c_ulong, ctypes.c_int, ctypes.c_ulong]
x11.XFlush.argtypes = [ctypes.c_void_p]
xtst.XTestFakeButtonEvent.argtypes = [ctypes.c_void_p, ctypes.c_uint, ctypes.c_int, ctypes.c_ulong]
xtst.XTestFakeMotionEvent.argtypes = [ctypes.c_void_p, ctypes.c_int, ctypes.c_int, ctypes.c_int, ctypes.c_ulong]




def display_oeffnen():
    """Verbindung erst aufbauen, wenn der Xvfb läuft."""
    global DISPLAY_HANDLE
    if DISPLAY_HANDLE is None:
        DISPLAY_HANDLE = x11.XOpenDisplay(DISPLAY.encode())
        if not DISPLAY_HANDLE:
            sys.exit(f"kein Display {DISPLAY}")
    return DISPLAY_HANDLE


def fenster() -> int | None:
    out = subprocess.run(
        ["xwininfo", "-root", "-children"],
        env={**os.environ, "DISPLAY": DISPLAY},
        capture_output=True, text=True,
    ).stdout
    for line in out.splitlines():
        if "jjcad" in line.lower():
            teile = line.split()
            for t in teile:
                if t.startswith("0x"):
                    return int(t, 16)
    return None


def fenster_versatz(wid: int) -> tuple[int, int]:
    """Fensterposition auf dem Bildschirm (Klickkoordinaten sind absolut)."""
    out = subprocess.run(
        ["xwininfo", "-id", hex(wid)],
        env={**os.environ, "DISPLAY": DISPLAY},
        capture_output=True, text=True,
    ).stdout
    ox = oy = 0
    for line in out.splitlines():
        if "Absolute upper-left X" in line:
            ox = int(line.split(":")[1])
        elif "Absolute upper-left Y" in line:
            oy = int(line.split(":")[1])
    return ox, oy


def klick(x: int, y: int, wid: int) -> None:
    dpy = display_oeffnen()
    x11.XSetInputFocus(ctypes.c_void_p(dpy), ctypes.c_ulong(wid), 2, 0)
    ox, oy = fenster_versatz(wid)
    xtst.XTestFakeMotionEvent(ctypes.c_void_p(dpy), -1, x + ox, y + oy, 0)
    x11.XFlush(ctypes.c_void_p(dpy))
    time.sleep(0.25)
    xtst.XTestFakeButtonEvent(ctypes.c_void_p(dpy), 1, 1, 0)
    x11.XFlush(ctypes.c_void_p(dpy))
    time.sleep(0.08)
    xtst.XTestFakeButtonEvent(ctypes.c_void_p(dpy), 1, 0, 0)
    x11.XFlush(ctypes.c_void_p(dpy))
    time.sleep(0.5)


def ziehen(x0: int, y0: int, x1: int, y1: int, wid: int) -> None:
    """Griff ziehen: drücken, in Schritten bewegen, loslassen."""
    dpy = display_oeffnen()
    x11.XSetInputFocus(ctypes.c_void_p(dpy), ctypes.c_ulong(wid), 2, 0)
    ox, oy = fenster_versatz(wid)
    xtst.XTestFakeMotionEvent(ctypes.c_void_p(dpy), -1, x0 + ox, y0 + oy, 0)
    x11.XFlush(ctypes.c_void_p(dpy))
    time.sleep(0.3)
    xtst.XTestFakeButtonEvent(ctypes.c_void_p(dpy), 1, 1, 0)
    x11.XFlush(ctypes.c_void_p(dpy))
    time.sleep(0.15)
    schritte = 12
    for k in range(1, schritte + 1):
        x = x0 + (x1 - x0) * k // schritte
        y = y0 + (y1 - y0) * k // schritte
        xtst.XTestFakeMotionEvent(ctypes.c_void_p(dpy), -1, x + ox, y + oy, 0)
        x11.XFlush(ctypes.c_void_p(dpy))
        time.sleep(0.05)
    time.sleep(0.15)
    xtst.XTestFakeButtonEvent(ctypes.c_void_p(dpy), 1, 0, 0)
    x11.XFlush(ctypes.c_void_p(dpy))
    time.sleep(0.5)


def taste(name: str, wid: int) -> None:
    """Einzelne Taste senden (z. B. Escape)."""
    dpy = display_oeffnen()
    x11.XSetInputFocus(ctypes.c_void_p(dpy), ctypes.c_ulong(wid), 2, 0)
    ks = x11.XStringToKeysym(name.encode())
    kc = x11.XKeysymToKeycode(ctypes.c_void_p(dpy), ctypes.c_ulong(ks))
    xtst.XTestFakeKeyEvent.argtypes = [ctypes.c_void_p, ctypes.c_uint, ctypes.c_int, ctypes.c_ulong]
    xtst.XTestFakeKeyEvent(ctypes.c_void_p(dpy), kc, 1, 0)
    x11.XFlush(ctypes.c_void_p(dpy))
    time.sleep(0.05)
    xtst.XTestFakeKeyEvent(ctypes.c_void_p(dpy), kc, 0, 0)
    x11.XFlush(ctypes.c_void_p(dpy))
    time.sleep(0.4)


def zeiger(wid: int) -> tuple[int, int, int, int]:
    """Zeigerposition: (root_x, root_y, win_x, win_y)."""
    dpy = display_oeffnen()
    rx, ry = ctypes.c_int(), ctypes.c_int()
    wx, wy = ctypes.c_int(), ctypes.c_int()
    mask = ctypes.c_uint()
    child = ctypes.c_ulong()
    x11.XQueryPointer(
        ctypes.c_void_p(dpy), ctypes.c_ulong(wid),
        ctypes.byref(ctypes.c_ulong()), ctypes.byref(child),
        ctypes.byref(rx), ctypes.byref(ry),
        ctypes.byref(wx), ctypes.byref(wy), ctypes.byref(mask),
    )
    return rx.value, ry.value, wx.value, wy.value


def schuss(pfad: str, wid: int) -> None:
    subprocess.run(
        ["import", "-window", hex(wid), pfad],
        env={**os.environ, "DISPLAY": DISPLAY}, check=True,
    )


def main() -> int:
    ziel = os.environ.get("OUT", "/tmp/klick-treppe")
    os.makedirs(ziel, exist_ok=True)
    xvfb = subprocess.Popen(
        [os.environ.get("XVFB", "Xvfb"), DISPLAY, "-screen", "0", f"{W}x{H}x24"],
        stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
    )
    time.sleep(1.5)
    display_oeffnen()
    app = subprocess.Popen(
        [BIN, DATEI],
        env={**os.environ, "DISPLAY": DISPLAY, "LIBGL_ALWAYS_SOFTWARE": "1",
             "WAYLAND_DISPLAY": ""},
        stdout=subprocess.DEVNULL, stderr=open("/tmp/jjcad_klick_treppe.log", "w"),
    )
    try:
        wid = None
        for _ in range(40):
            time.sleep(1.0)
            wid = fenster()
            if wid:
                break
        if not wid:
            print("kein Fenster")
            return 1
        time.sleep(3.0)
        schuss(f"{ziel}/treppe_vorher.png", wid)
        # Werkzeug Treppe über die Taste B starten
        taste("b", wid)
        time.sleep(0.6)
        # Drei Klicks: Lauflinie Anfang, Ende, Breite
        klick(420, 700, wid)   # Anfang
        klick(420, 300, wid)   # Ende (4,6 m bei 1:100)
        klick(470, 500, wid)   # Breite (Abstand zur Lauflinie)
        time.sleep(1.0)
        schuss(f"{ziel}/treppe_gesetzt.png", wid)
        # Treppe anklicken — Eigenschaften-Panel mit Auftritt/Steigung
        klick(420, 500, wid)
        time.sleep(0.8)
        schuss(f"{ziel}/treppe_auswahl.png", wid)
        # Knopf „Beschriftung erzeugen" im Eigenschaften-Panel
        klick(1354, 826, wid)
        time.sleep(0.8)
        schuss(f"{ziel}/treppe_beschriftet.png", wid)
        # Rechte Längskante nach rechts ziehen: nur diese Kante wandert
        # (Parallelverschiebung), die Treppe wird breiter.
        ziehen(470, 500, 545, 500, wid)
        # Im Modus „Zwei Klicks" beendet der nächste Klick den Zug.
        klick(545, 500, wid)
        time.sleep(0.8)
        schuss(f"{ziel}/treppe_breiter.png", wid)
        # Ein Klick in die Fläche darf die Treppe NICHT verschieben (nur die
        # Griffe an den Kanten bewegen sie).
        ziehen(470, 560, 550, 560, wid)
        klick(550, 560, wid)
        time.sleep(0.8)
        schuss(f"{ziel}/treppe_klick_zieht_nicht.png", wid)
        print("Bilder:", ziel)
        return 0
    finally:
        app.terminate()
        try:
            app.wait(timeout=5)
        except subprocess.TimeoutExpired:
            app.kill()
        if xvfb is not None:
            xvfb.terminate()
        try:
            xvfb.wait(timeout=5)
        except subprocess.TimeoutExpired:
            xvfb.kill()


if __name__ == "__main__":
    sys.exit(main())
