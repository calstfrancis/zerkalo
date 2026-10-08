import random, re, os
exec(open(S_SETUP).read())
rng = random.Random(int(os.environ.get("SEED", "7")))
N = int(os.environ.get("STEPS", "120"))
EDX = (450, 980); EDY = (262, 868)
def p(): return rng.randint(*EDX), rng.randint(*EDY)
away = {"text": p, "preview": lambda: (rng.randint(1355, 1395), rng.randint(300, 800)),
        "sidebar": lambda: (rng.randint(20, 380), rng.randint(500, 760)),
        "toolbar": lambda: (rng.randint(520, 900), 140)}
def act_click():      x, y = p(); click(x, y); return f'{x},{y}'
def act_rclick_away():
    x, y = p(); rclick(x, y); sleep(0.6)
    k = rng.choice(list(away)); ax, ay = away[k](); click(ax, ay); return f'menu@{x},{y} -> {k}@{ax},{ay}'
def act_rclick_esc(): x, y = p(); rclick(x, y); sleep(0.6); key("Escape"); return f'{x},{y}'
def act_dblclick():   x, y = p(); click(x, y, n=2); return f'{x},{y}'
def act_focus_out_in():
    k = rng.choice(["preview", "sidebar"]); ax, ay = away[k]()
    if CUR[0] in WATCH: shot(f"fz{CUR[0]}_0.png")
    click(ax, ay); sleep(0.4)
    if CUR[0] in WATCH: shot(f"fz{CUR[0]}_1.png")
    x, y = p(); click(x, y)
    if CUR[0] in WATCH: sleep(1.2); shot(f"fz{CUR[0]}_2.png")
    return f'{k}@{ax},{ay} back@{x},{y}'
def act_type():       type_(rng.choice(["word ", "abc", "- ", "x", "the fox "]))
def act_enter():      key("Return")
def act_back():       key("BackSpace", "BackSpace")
def act_undo():       key("ctrl+z")
def act_wheel():      wheel(rng.choice([-4, -2, 2, 4]), *p())
clicky = [act_click, act_rclick_away, act_rclick_esc, act_dblclick, act_focus_out_in]
typing = [act_type, act_enter, act_back, act_undo]
acts = clicky * 3 + typing * 2 + [act_wheel]
click(700, 560); key("ctrl+Home"); sleep(0.5); wheel(20, 700, 560); sleep(2.5)
flags = 0
CUR = [0]
WATCH = set()
for i in range(N):
    CUR[0] = i
    a = rng.choice(acts)
    sleep(1.2); mark()                       # let any previous scroll finish first
    info = a()
    sleep(1.2)
    vals = [int(m.group(1)) for l in traces()[_mark[0]:] for m in [re.search(r"v=(\d+)", l)] if m]
    moved = (max(vals) - min(vals)) if vals else 0
    raw = [l.split('SCROLLTRACE ')[1].strip() for l in traces()[_mark[0]:]]
    print(f"   step {i} {a.__name__} {info or ''} -> " + ('; '.join(dict.fromkeys(raw)) if raw else '-'), flush=True)
    kind = "click" if a in clicky else ("wheel" if a is act_wheel else "type")
    if kind == "click" and vals:
        flags += 1
        print(f"!! step {i} {a.__name__}{'('+info+')' if info else ''}: scrolled {vals[0]}..{vals[-1]} (span {moved})", flush=True)
        shot(f"fuzz_{i}.png")
print(f"== fuzz done: {N} steps, {flags} click/menu actions moved the view", flush=True)
