import gi
gi.require_version("Gtk","4.0"); gi.require_version("GtkSource","5")
from gi.repository import Gtk, GtkSource
Gtk.init()
b = GtkSource.Buffer(); v = GtkSource.View(buffer=b)
w = Gtk.Window(); w.set_child(v); w.present()
b.set_text("hello world\n"); b.set_enable_undo(True)
b.set_modified(False)
log=[]
b.connect("modified-changed", lambda buf: log.append(("modified-changed", buf.get_modified())))
b.place_cursor(b.get_end_iter())
b.begin_user_action(); b.insert_at_cursor("X"); b.end_user_action()
print("after typing  modified =", b.get_modified())
b.undo()
print("after undo    modified =", b.get_modified())
b.redo(); print("after redo    modified =", b.get_modified())
b.set_modified(False); b.undo(); print("save then undo: modified =", b.get_modified()); b.redo(); print("then redo: modified =", b.get_modified())
print(log)
