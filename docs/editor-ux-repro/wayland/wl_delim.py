exec(open(S_SETUP).read())
click(700, 560); key("ctrl+End"); sleep(0.4)
key("Return", "Return"); type_("a *b* c and $x$ d"); sleep(0.5)
key("Left", "Left", "Left", "Left", "Left", "Left", "Left", "Left", "Left", "Left", "Left", "Left"); sleep(1.0)   # just after the closing *
shot("wl_d1.png")
key("Left", "Left"); sleep(1.0)                                                                                     # before the opening *? cursor after 'a ' -> at '*'
shot("wl_d2.png")
for _ in range(8): key("Right")
sleep(1.0); shot("wl_d3.png")
