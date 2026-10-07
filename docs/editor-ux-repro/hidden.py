import gi
import os; HERE = os.path.dirname(os.path.abspath(__file__))
gi.require_version("Gtk","4.0"); gi.require_version("GtkSource","5")
from gi.repository import Gtk, GtkSource, GLib
Gtk.init()
b = GtkSource.Buffer(); v = GtkSource.View(buffer=b)
w = Gtk.Window(); w.set_default_size(500,300); sw=Gtk.ScrolledWindow(); sw.set_child(v); w.set_child(sw); w.present()
pre = '#set page(margin: 1in)\n#set text(font: "X")\n// ── Document body\n'
b.set_text(pre + "Hello body.\nSecond.\n")
t = b.create_tag("hid", invisible=True, editable=False)
b.apply_tag(t, b.get_start_iter(), b.get_iter_at_offset(len(pre)))
v.grab_focus()
def body_start(): return b.get_iter_at_offset(len(pre))
def report(label):
    s,e=b.get_bounds(); txt=b.get_text(s,e,True)
    print(f"{label}: cursor={b.get_iter_at_mark(b.get_insert()).get_offset()} (body starts {len(pre)}) preamble_intact={txt.startswith(pre)}")
def run():
    b.place_cursor(body_start()); v.emit("backspace"); report("backspace at body start")
    b.set_text(pre + "Hello body.\nSecond.\n"); b.apply_tag(t, b.get_start_iter(), body_start())
    v.emit("move-cursor", Gtk.MovementStep.BUFFER_ENDS, -1, False); report("Ctrl+Home")
    v.emit("insert-at-cursor", "typed"); s,e=b.get_bounds(); print("  text now begins:", repr(b.get_text(s,e,True)[:30]))
    b.set_text(pre + "Hello body.\nSecond.\n"); b.apply_tag(t, b.get_start_iter(), body_start())
    b.place_cursor(b.get_iter_at_offset(len(pre)+3)); v.emit("move-cursor", Gtk.MovementStep.DISPLAY_LINES, -1, False); report("Up arrow from first body line")
    v.emit("select-all", True); sel=b.get_selection_bounds(); print("  select-all from", sel[0].get_offset())
    w.close(); loop.quit()
GLib.timeout_add(300, lambda: (run(), False)[1])
loop=GLib.MainLoop(); loop.run()
