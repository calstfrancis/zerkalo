"""Drive a headless GNOME Shell (Wayland) through mutter's RemoteDesktop/ScreenCast
D-Bus API: real compositor-level pointer/keyboard events and screenshots.
usage: python3 wl_drive.py <steps.py>   (steps get move/click/rclick/key/type/wheel/shot/sleep/mark/report)
"""
import os, sys, time, re
import gi
gi.require_version("Gst", "1.0")
gi.require_version("Gdk", "4.0")
from gi.repository import Gio, GLib, Gst, Gdk

S = os.environ.get("OUT", os.path.dirname(os.path.abspath(__file__)))
LOG = os.environ.get("APPLOG", os.path.join(os.path.dirname(os.path.abspath(__file__)), "app.log"))
bus = Gio.bus_get_sync(Gio.BusType.SESSION)


def call(dest, path, iface, method, args, ret=None):
    return bus.call_sync(dest, path, iface, method, args,
                         GLib.VariantType(ret) if ret else None,
                         Gio.DBusCallFlags.NONE, 10000, None)


RD = "org.gnome.Mutter.RemoteDesktop"
SC = "org.gnome.Mutter.ScreenCast"
rd_path = call(RD, "/org/gnome/Mutter/RemoteDesktop", RD, "CreateSession", None, "(o)")[0]
sid = bus.call_sync(RD, rd_path, "org.freedesktop.DBus.Properties", "Get",
                    GLib.Variant("(ss)", (RD + ".Session", "SessionId")),
                    GLib.VariantType("(v)"), 0, 5000, None)[0]
sc_path = call(SC, "/org/gnome/Mutter/ScreenCast", SC, "CreateSession",
               GLib.Variant("(a{sv})", ({"remote-desktop-session-id": GLib.Variant("s", sid)},)), "(o)")[0]
stream = call(SC, sc_path, SC + ".Session", "RecordMonitor",
              GLib.Variant("(sa{sv})", (os.environ.get("MONITOR", "Meta-0"), {})), "(o)")[0]
node = {}
bus.signal_subscribe(SC, SC + ".Stream", "PipeWireStreamAdded", stream, None, 0,
                     lambda *a: node.setdefault("id", a[5][0]))
call(RD, rd_path, RD + ".Session", "Start", None)
ctx = GLib.MainContext.default()
deadline = time.time() + 10
while "id" not in node and time.time() < deadline:
    ctx.iteration(False)
    time.sleep(0.02)
Gst.init(None)


def pump(sec):
    end = time.time() + sec
    while time.time() < end:
        ctx.iteration(False)
        time.sleep(0.01)


def rdcall(method, sig, *args):
    call(RD, rd_path, RD + ".Session", method, GLib.Variant(sig, args))


def move(x, y):
    rdcall("NotifyPointerMotionAbsolute", "(sdd)", stream, float(x), float(y))
    pump(0.05)


def button(code, down):
    rdcall("NotifyPointerButton", "(ib)", code, down)


def click(x=None, y=None, n=1, btn=272, hold=0.06):
    if x is not None:
        move(x, y)
    for _ in range(n):
        button(btn, True); pump(hold); button(btn, False); pump(0.08)


def rclick(x=None, y=None):
    click(x, y, btn=273)


def wheel(steps, x=None, y=None):
    if x is not None:
        move(x, y)
    for _ in range(abs(steps)):
        rdcall("NotifyPointerAxisDiscrete", "(ui)", 0, 1 if steps > 0 else -1)
        pump(0.04)


MODS = {"ctrl": "Control_L", "shift": "Shift_L", "alt": "Alt_L", "super": "Super_L"}


def keysym(name):
    return Gdk.keyval_from_name(MODS.get(name, name))


def key(*combos):
    for combo in combos:
        parts = combo.split("+")
        syms = [keysym(p) for p in parts]
        for k in syms:
            rdcall("NotifyKeyboardKeysym", "(ub)", k, True); pump(0.02)
        for k in reversed(syms):
            rdcall("NotifyKeyboardKeysym", "(ub)", k, False); pump(0.02)
        pump(0.05)


def type_(text):
    for ch in text:
        k = Gdk.unicode_to_keyval(ord(ch))
        rdcall("NotifyKeyboardKeysym", "(ub)", k, True)
        rdcall("NotifyKeyboardKeysym", "(ub)", k, False)
        pump(0.04)


def shot(name):
    p = Gst.parse_launch(
        f"pipewiresrc path={node['id']} num-buffers=4 ! videoconvert ! pngenc snapshot=true "
        f"! filesink location={os.path.join(S, name)}")
    p.set_state(Gst.State.PLAYING)
    p.get_bus().timed_pop_filtered(5 * Gst.SECOND, Gst.MessageType.EOS | Gst.MessageType.ERROR)
    p.set_state(Gst.State.NULL)


_mark = [0]


def traces():
    try:
        return [l for l in open(LOG, errors="replace") if "SCROLLTRACE" in l]
    except FileNotFoundError:
        return []


def mark():
    _mark[0] = len(traces())


def report(label):
    vals = []
    for l in traces()[_mark[0]:]:
        m = re.search(r"v=(\d+)", l)
        if m and (not vals or vals[-1] != m.group(1)):
            vals.append(m.group(1))
    print(f"== {label}: " + (" ".join(vals[:14]) if vals else "(no scroll)"), flush=True)
    mark()


def sleep(s):
    pump(s)


S_SETUP = os.path.join(os.path.dirname(os.path.abspath(__file__)), "wl_setup.py")
env = dict(rdcall=rdcall, keysym=keysym, _mark=_mark, S_SETUP=S_SETUP, move=move, click=click, rclick=rclick, key=key, type_=type_, wheel=wheel,
           shot=shot, sleep=sleep, mark=mark, report=report, traces=traces)
exec(open(sys.argv[1]).read(), env)
call(RD, rd_path, RD + ".Session", "Stop", None)
