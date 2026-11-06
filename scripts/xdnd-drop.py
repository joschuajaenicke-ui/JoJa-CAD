#!/usr/bin/env python3
"""Zieht Dateien synthetisch auf ein X11-Fenster (XDND) — ohne Maus.

Wozu: Datei-Drop lässt sich headless nur prüfen, wenn irgendetwas den Zug
auslöst. Dieses Skript ist die **Quelle** des Zuges und spricht das
XDND-Protokoll so, wie es ein Dateimanager täte:

    XdndEnter  ->  XdndPosition  ->  SelectionRequest (text/uri-list) bedienen
               ->  XdndDrop    ->  XdndFinished abwarten

Damit ist prüfbar, ob ein Programm Drops annimmt und was es damit tut.
Siehe docs/WAYLAND-DATEI-DROP.md — unter Wayland (nativ) gibt es kein XDND,
dort ist dieses Skript wirkungslos; es prüft den X11-Zweig.

Voraussetzung: python-xlib  (pip install python-xlib)

Aufruf:
    DISPLAY=:99 python3 scripts/xdnd-drop.py "JoJa-CAD" samples/Bogenprobe.jjcad
    DISPLAY=:99 python3 scripts/xdnd-drop.py --display :99 "JoJa-CAD" a.jjcad b.dxf

Rückgabewerte: 0 = Zug vollständig (XdndFinished), 2 = Aufruffehler,
3 = Zielfenster nicht gefunden, 4 = Ziel hat die Auswahl nicht angefordert.
"""

import argparse
import os
import sys
import time

from Xlib import X, display, protocol


def atom(d, name):
    return d.intern_atom(name)


def finde_fenster(d, root, titel):
    """Sucht ein Fenster, dessen WM_NAME den Text enthält (Tiefensuche)."""

    def lauf(w, tiefe=0):
        if tiefe > 8:
            return None
        try:
            name = w.get_wm_name()
        except Exception:
            name = None
        if name and titel in str(name):
            return w
        try:
            kinder = w.query_tree().children
        except Exception:
            kinder = []
        for k in kinder:
            treffer = lauf(k, tiefe + 1)
            if treffer is not None:
                return treffer
        return None

    return lauf(root)


def main():
    p = argparse.ArgumentParser(description="Synthetischer XDND-Datei-Drop.")
    p.add_argument("titel", help="Text im Fenstertitel des Ziels")
    p.add_argument("dateien", nargs="+", help="abzulegende Dateien")
    p.add_argument("--display", default=os.environ.get("DISPLAY", ":99"))
    p.add_argument("--ruhe", type=float, default=0.3,
                   help="Wartezeit zwischen Position und Drop in Sekunden")
    args = p.parse_args()

    pfade = [os.path.abspath(f) for f in args.dateien]
    for f in pfade:
        if not os.path.exists(f):
            print("Datei fehlt:", f)
            return 2
    uris = "".join("file://" + f + "\r\n" for f in pfade).encode("utf-8")

    d = display.Display(args.display)
    root = d.screen().root
    ziel = finde_fenster(d, root, args.titel)
    if ziel is None:
        print("Zielfenster nicht gefunden:", args.titel)
        return 3
    print("Ziel:", hex(ziel.id))

    A = {n: atom(d, n) for n in [
        "XdndEnter", "XdndPosition", "XdndStatus", "XdndLeave", "XdndDrop",
        "XdndFinished", "XdndSelection", "XdndActionCopy", "XdndTypeList",
        "text/uri-list",
    ]}
    aware = ziel.get_full_property(atom(d, "XdndAware"), X.AnyPropertyType)
    print("XdndAware:", None if aware is None else aware.value[0])

    # Eigene Quelle: ein kleines, nicht abgebildetes Fenster, das die Auswahl
    # hält. Ohne Eigentümer von XdndSelection kann das Ziel die Pfade nicht
    # abholen.
    quelle = root.create_window(
        -10, -10, 10, 10, 0, d.screen().root_depth, X.InputOutput, X.CopyFromParent
    )
    quelle.set_wm_name("xdnd-quelle")
    quelle.set_selection_owner(A["XdndSelection"], X.CurrentTime)
    d.flush()

    def sende(typ, daten):
        daten = list(daten) + [0] * (5 - len(daten))
        ev = protocol.event.ClientMessage(
            window=ziel, client_type=typ, data=(32, daten[:5])
        )
        ziel.send_event(ev, event_mask=X.NoEventMask)
        d.flush()

    # Ein Ziel, das schon einen Zug gesehen hat, setzt seinen Zustand erst mit
    # XdndLeave zurück (winit: dnd.reset()).
    sende(A["XdndLeave"], [quelle.id, 0, 0, 0, 0])
    time.sleep(0.1)
    # XdndEnter: Version 5, keine Zusatztypen (Bit 0 in data[1] bleibt aus)
    sende(A["XdndEnter"], [quelle.id, 5 << 24, A["text/uri-list"], 0, 0])
    time.sleep(0.2)
    # XdndPosition: Koordinaten in Wurzelkoordinaten, Zeit 0 = CurrentTime
    sende(A["XdndPosition"],
          [quelle.id, 0, (100 << 16) | 100, X.CurrentTime, A["XdndActionCopy"]])

    bedient = False
    status = None
    ende = time.time() + 5.0
    while time.time() < ende:
        d.flush()
        while d.pending_events():
            ev = d.next_event()
            if ev.type == X.SelectionRequest:
                prop = ev.property if ev.property != X.NONE else ev.target
                ev.requestor.change_property(prop, A["text/uri-list"], 8, uris)
                notify = protocol.event.SelectionNotify(
                    time=ev.time, requestor=ev.requestor, selection=ev.selection,
                    target=ev.target, property=prop,
                )
                ev.requestor.send_event(notify, event_mask=0)
                d.flush()
                bedient = True
                print("SelectionRequest bedient")
            elif ev.type == X.ClientMessage and ev.client_type == A["XdndStatus"]:
                status = ev.data[1]
                print("XdndStatus akzeptiert:", status)
        if bedient and status is not None:
            break
        time.sleep(0.02)

    if not bedient:
        print("keine SelectionRequest erhalten — Ziel nimmt keine Dateien an")
        return 4

    # Das Ziel wertet die Auswahl im SelectionNotify aus; erst danach darf der
    # Drop kommen, sonst ist die Pfadliste noch leer.
    time.sleep(args.ruhe)
    sende(A["XdndDrop"], [quelle.id, 0, X.CurrentTime])
    print("XdndDrop gesendet")

    ende = time.time() + 3.0
    fertig = False
    while time.time() < ende and not fertig:
        d.flush()
        while d.pending_events():
            ev = d.next_event()
            if ev.type == X.ClientMessage and ev.client_type == A["XdndFinished"]:
                print("XdndFinished, akzeptiert:", ev.data[1])
                fertig = True
        time.sleep(0.02)
    print("fertig" if fertig else "kein XdndFinished")
    return 0


if __name__ == "__main__":
    sys.exit(main())
