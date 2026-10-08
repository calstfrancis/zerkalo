import os
exec(open(S_SETUP).read())
W = os.environ["W"]
click(700, 560); key("ctrl+End"); sleep(0.4)
key("Return", "Return"); type_("see the *strong* point and $x$ here"); key("Return"); type_("https://example.org/page")
key("shift+Home"); sleep(0.2); key("ctrl+c"); sleep(0.3)
key("BackSpace"); key("Up", "Home"); sleep(0.3)
for _ in range(4): key("ctrl+Right")      # select the words "see the" ... via shift
key("Home"); key("shift+ctrl+Right", "shift+ctrl+Right"); sleep(0.3)
shot("wl_w_sel.png")
key("ctrl+v"); sleep(1.5); shot("wl_w_after.png")
# delimiter highlight: put the cursor right after the opening * of *strong*
key("Home"); 
for _ in range(12): key("Right")
sleep(1.0); shot("wl_w_delim.png")
sleep(3.5)
print("FILE:", [l for l in open(os.path.join(W, "main.typ")).read().splitlines()[-4:]], flush=True)
