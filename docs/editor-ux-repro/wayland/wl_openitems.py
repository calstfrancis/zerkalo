import os
exec(open(S_SETUP).read())
def title(name):
    shot(name)
click(700, 560); key("ctrl+End"); sleep(0.6)
title("oi_0_clean.png")
type_("x"); sleep(0.5); title("oi_1_typed.png")
key("ctrl+z"); sleep(0.6); title("oi_2_undone.png")
key("ctrl+shift+z"); sleep(0.6); title("oi_3_redone.png")
# hover delay: make a compile error and hover it
key("Return", "Return"); type_("#nosuchfunction()"); sleep(0.4)
key("ctrl+shift+p"); sleep(6.0)
shot("oi_4_error_marked.png")
