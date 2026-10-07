import gi, sys, subprocess, os
import os; HERE = os.path.dirname(os.path.abspath(__file__))
gi.require_version("Gtk","4.0"); gi.require_version("GtkSource","5")
from gi.repository import Gtk, GtkSource, GLib
MODE = sys.argv[1]; JIT = sys.argv[2]
Gtk.init()
b = GtkSource.Buffer(); v = GtkSource.View(buffer=b); v.set_monospace(True)
sw = Gtk.ScrolledWindow(); sw.set_child(v)
w = Gtk.Window(); w.set_default_size(600, 400); w.set_child(sw); w.set_decorated(False); w.present()
b.set_text("".join(f"line {i} lorem ipsum dolor sit amet\n" for i in range(300)))
flag = [False]
def on_changed(_):
    if MODE == "buggy": flag[0] = True
b.connect("changed", on_changed)
def on_mark(buf, it, m):
    if m.get_name() != "insert": return
    was = flag[0]; flag[0] = False
    if was and not buf.get_has_selection():
        def idle():
            if not buf.get_has_selection():
                v.scroll_to_mark(buf.get_insert(), 0.15, False, 0.0, 0.5)
        GLib.idle_add(idle)
b.connect("mark-set", on_mark)
def step(n=[0]):
    n[0]+=1
    if n[0]==1:
        it = b.get_iter_at_line(100)[1]; b.place_cursor(it)
        v.scroll_to_iter(it, 0.0, True, 0.0, 0.3); return True
    if n[0]==2:
        v.grab_focus(); v.emit("insert-at-cursor", "typed "); return True
    if n[0]==3:
        x = sw.get_vadjustment().get_value(); print("vadj before click", x)
        # click near the bottom edge (y=385 of 400), with a tiny 2px jitter while held
        open(os.path.join(HERE, 'go'),"w").write("xdotool mousemove 200 385 mousedown 1 sleep 0.05 mousemove 202 386 sleep 0.05 mouseup 1".replace("202 386", JIT))
        return True
    if n[0]==8:
        print("vadj after click", sw.get_vadjustment().get_value())
        s = b.get_selection_bounds()
        print(MODE, "selection:", repr(b.get_text(s[0], s[1], False)) if s else None)
        w.close(); return False
    return True
GLib.timeout_add(300, step)
loop = GLib.MainLoop(); w.connect("close-request", lambda *_: (loop.quit(), False)[1]); loop.run()
