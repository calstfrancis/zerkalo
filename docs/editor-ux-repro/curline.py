import gi
import os; HERE = os.path.dirname(os.path.abspath(__file__))
gi.require_version("Gtk","4.0"); gi.require_version("GtkSource","5")
from gi.repository import Gtk, GtkSource, GLib
Gtk.init()
css = Gtk.CssProvider(); css.load_from_string("textview text .current-line { background-color: alpha(@accent_color, 0.10); }")
from gi.repository import Gdk
Gtk.StyleContext.add_provider_for_display(Gdk.Display.get_default(), css, 600)
b = GtkSource.Buffer(); b.set_style_scheme(GtkSource.StyleSchemeManager.get_default().get_scheme("Adwaita"))
v = GtkSource.View(buffer=b); v.set_wrap_mode(Gtk.WrapMode.WORD_CHAR); v.set_highlight_current_line(True); v.set_left_margin(40)
w = Gtk.Window(); w.set_default_size(500,320); w.set_child(v); w.set_decorated(False); w.present()
para = "This is a long prose paragraph written the way people write essays, and it wraps across several display lines because the editor never scrolls sideways. "*3
b.set_text("= Heading\n\n" + para + "\n\nNext paragraph.\n")
def step(n=[0]):
    n[0]+=1
    if n[0]==1: v.grab_focus(); b.place_cursor(b.get_iter_at_offset(40)); return True
    if n[0]==3: open(os.path.join(HERE, 'go'),"w").write("import -window root -crop 500x320+0+0 " + os.path.join(HERE, 'curline.png')); return True
    if n[0]==6: w.close(); return False
    return True
GLib.timeout_add(300, step)
loop=GLib.MainLoop(); w.connect("close-request", lambda *_: (loop.quit(), False)[1]); loop.run()
