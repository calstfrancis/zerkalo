import gi, sys, subprocess
import os; HERE = os.path.dirname(os.path.abspath(__file__))
gi.require_version("Gtk","4.0"); gi.require_version("GtkSource","5")
from gi.repository import Gtk, GtkSource, GLib
Gtk.init()
b = GtkSource.Buffer(); v = GtkSource.View(buffer=b); v.set_monospace(True)
sw = Gtk.ScrolledWindow(vexpand=True); sw.set_child(v)
e = Gtk.Entry()
box = Gtk.Box(orientation=Gtk.Orientation.VERTICAL); box.append(e); box.append(sw)
w = Gtk.Window(); w.set_default_size(600, 430); w.set_child(box); w.set_decorated(False); w.present()
b.set_text("".join(f"line {i} lorem ipsum dolor sit amet\n" for i in range(300)))
changes=[]
sw.get_vadjustment().connect("value-changed", lambda a: changes.append(round(a.get_value())))
def step(n=[0]):
    n[0]+=1
    if n[0]==1:
        b.place_cursor(b.get_iter_at_line(5)[1]); v.grab_focus(); return True
    if n[0]==2:
        sw.get_vadjustment().set_value(2400); e.grab_focus(); changes.clear(); return True
    if n[0]==3:
        open(os.path.join(HERE, 'go'),"w").write("xdotool mousemove 200 250 mousedown 1 sleep 0.08 mouseup 1"); return True
    if n[0]==6:
        print("adjustment changes during click:", changes)
        it = b.get_iter_at_mark(b.get_insert()); print("cursor line", it.get_line(), "sel", b.get_has_selection())
        w.close(); return False
    return True
GLib.timeout_add(300, step)
loop = GLib.MainLoop(); w.connect("close-request", lambda *_: (loop.quit(), False)[1]); loop.run()
