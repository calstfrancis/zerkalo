import gi
import os; HERE = os.path.dirname(os.path.abspath(__file__))
gi.require_version("Gtk","4.0"); gi.require_version("GtkSource","5")
from gi.repository import Gtk, GtkSource, GLib, Gdk
Gtk.init()
b = GtkSource.Buffer(); v = GtkSource.View(buffer=b); v.set_wrap_mode(Gtk.WrapMode.WORD_CHAR)
sw = Gtk.ScrolledWindow(); sw.set_child(v)
w = Gtk.Window(); w.set_default_size(600,400); w.set_child(sw); w.set_decorated(False); w.present()
b.set_text("".join(f"line {i} lorem ipsum dolor sit amet\n" for i in range(400)))
vals=[]
sw.get_vadjustment().connect("value-changed", lambda a: vals.append(round(a.get_value())))
def step(n=[0]):
    n[0]+=1
    if n[0]==1:
        it=b.get_iter_at_line(200)[1]; b.place_cursor(it); v.scroll_to_iter(it,0,True,0,0.5); v.grab_focus(); return True
    if n[0]==2:
        v.get_clipboard().set("PASTED text\nmore\n"); vals.clear(); print("before", round(sw.get_vadjustment().get_value()))
        v.emit("paste-clipboard"); return True
    if n[0]==6: print("adjustment after paste:", vals[:20], "final", round(sw.get_vadjustment().get_value())); w.close(); return False
    return True
GLib.timeout_add(300, step)
loop=GLib.MainLoop(); w.connect("close-request", lambda *_: (loop.quit(), False)[1]); loop.run()
