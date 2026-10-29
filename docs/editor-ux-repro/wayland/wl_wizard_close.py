import os
exec(open(S_SETUP).read())
# first launch, no git remote: the setup wizard opens. Close it with the window's own close (Alt+F4).
shot("wz_1_open.png")
key("alt+F4"); sleep(2); shot("wz_2_closed.png")
cfg = os.path.join(os.environ["HOME"], ".config", "zerkalo", "config.toml")
print("SETUP_DONE:", [l for l in open(cfg).read().splitlines() if "setup_done" in l] if os.path.exists(cfg) else "no config", flush=True)
