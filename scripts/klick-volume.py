#!/usr/bin/env python3
"""Klickprüfung: Volumen im Programmstandard (Einfügen per Punkten) anlegen.

Startet ein eigenes Xvfb + eine eigene JoJa-CAD-Instanz (die Instanz des
Nutzers bleibt unberührt), klickt die Volumen-Schaltfläche, setzt vier Punkte
und schließt mit Enter; Screenshots vor/nach der Eingabe.

Klicks über XTest, damit sie wie echte Zeigereignisse ankommen (egui wertet
Roh-Events aus).

    python3 scripts/klick-volume.py            # Punkte-Modus (Standard)
    MODE=contour python3 scripts/klick-volume.py
"""
import ctypes
import ctypes.util
import os
import subprocess
import sys
import time

DISPLAY = ":97"
W, H = 1600, 1000
BIN = os.environ.get("BIN", "target/debug/jjcad")
DATEI = os.environ.get("FILE", "samples/Volumen.jjcad")
MODE = os.environ.get("MODE", "points")

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
    os.makedirs("/home/joschua/jjcad-live/wand-probe", exist_ok=True)
    xvfb = subprocess.Popen(
        ["Xvfb", DISPLAY, "-screen", "0", f"{W}x{H}x24"],
        stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
    )
    time.sleep(1.5)
    display_oeffnen()
    app = subprocess.Popen(
        [BIN, DATEI],
        env={**os.environ, "DISPLAY": DISPLAY, "LIBGL_ALWAYS_SOFTWARE": "1",
             "WAYLAND_DISPLAY": ""},
        stdout=subprocess.DEVNULL, stderr=open("/tmp/jjcad_klick.log","w"),
    )
    try:
        wid = None
        for _ in range(30):
            time.sleep(1.0)
            wid = fenster()
            if wid:
                break
        if not wid:
            print("kein Fenster"); return 1
        time.sleep(3.0)
        schuss("/home/joschua/jjcad-live/wand-probe/klick_vorher.png", wid)
        # Reiter Architektur, dann die Volumen-Schaltfläche (Standardmodus)
        klick(345, 20, wid)
        time.sleep(0.6)
        klick(335, 75, wid)
        time.sleep(0.6)
        if MODE == "drag":
            # Volumen per Punkten anlegen, dann eine Ecke ziehen
            for x, y in [(350, 250), (520, 250), (520, 380), (350, 380)]:
                klick(x, y, wid)
            taste("Return", wid)
            time.sleep(0.8)
            # Volumen anklicken (auswählen), dann eine Ecke ziehen
            klick(430, 320, wid)
            time.sleep(0.6)
            schuss(f"/home/joschua/jjcad-live/wand-probe/klick_griff_vorher.png", wid)
            ziehen(520, 250, 640, 190, wid)
        elif MODE == "contour":
            klick(400, 555, wid)  # auf die waagerechte Wand
        else:
            # Vier Punkte im freien Bereich, dann Enter
            for x, y in [(350, 250), (520, 250), (520, 380), (350, 380)]:
                klick(x, y, wid)
            taste("Return", wid)
        time.sleep(1.2)
        schuss("/home/joschua/jjcad-live/wand-probe/klick_nachher.png", wid)
        print(f"Screenshots ({MODE}): klick_vorher.png, klick_nachher.png")
    finally:
        app.terminate()
        try:
            app.wait(timeout=5)
        except subprocess.TimeoutExpired:
            app.kill()
        xvfb.terminate()
    return 0


if __name__ == "__main__":
    sys.exit(main())
