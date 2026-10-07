import gi, sys
import os; HERE = os.path.dirname(os.path.abspath(__file__))
gi.require_version("Gtk","4.0"); gi.require_version("GtkSource","5")
from gi.repository import Gtk, GtkSource, GLib
Gtk.init()
b = GtkSource.Buffer(); v = GtkSource.View(buffer=b); v.set_show_line_numbers(True); v.set_left_margin(8); v.set_highlight_current_line(True)
sw = Gtk.ScrolledWindow(); sw.set_child(v)
w = Gtk.Window(); w.set_default_size(600, 400); w.set_child(sw); w.set_decorated(False); w.present()
b.set_text("".join(f"line {i} lorem ipsum dolor sit amet\n" for i in range(50)))
def step(n=[0]):
    n[0]+=1
    if n[0]==1:
        g = v.get_gutter(Gtk.TextWindowType.LEFT); print("gutter width", g.get_width())
        open(os.path.join(HERE, 'go'),"w").write("xdotool mousemove 10 60 click 1"); return True
    if n[0]==4:
        s=b.get_selection_bounds(); print("click on line number -> selection:", repr(b.get_text(s[0],s[1],False)) if s else None); w.close(); return False
    return True
GLib.timeout_add(300, step)
loop=GLib.MainLoop(); w.connect("close-request", lambda *_: (loop.quit(), False)[1]); loop.run()
