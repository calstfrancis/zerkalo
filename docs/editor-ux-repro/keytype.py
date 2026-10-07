import gi
import os; HERE = os.path.dirname(os.path.abspath(__file__))
gi.require_version("Gtk","4.0"); gi.require_version("GtkSource","5")
from gi.repository import Gtk, GtkSource, GLib
Gtk.init()
b = GtkSource.Buffer(); v = GtkSource.View(buffer=b)
w = Gtk.Window(); w.set_default_size(400,300); w.set_child(v); w.set_decorated(False); w.present()
b.set_text("hello\n"); log=[]
b.connect("changed", lambda *_: log.append("changed"))
b.connect("mark-set", lambda buf,it,m: log.append("mark-set:insert") if m.get_name()=="insert" else None)
def step(n=[0]):
    n[0]+=1
    if n[0]==1: v.grab_focus(); log.clear(); open(os.path.join(HERE, 'go'),"w").write("xdotool type --delay 50 abc; xdotool key BackSpace Delete Return"); return True
    if n[0]==6: print(log); w.close(); return False
    return True
GLib.timeout_add(300, step)
loop=GLib.MainLoop(); w.connect("close-request", lambda *_: (loop.quit(), False)[1]); loop.run()
