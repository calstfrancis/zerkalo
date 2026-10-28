//! The per-style heading rules. Every rule resets the size and weight Typst
//! gives a heading by default, so what a style asks for is what is drawn.

#[derive(Clone, Copy, Default)]
struct Level {
    center: bool,
    bold: bool,
    italic: bool,
    caps: bool,
    small_caps: bool,
    run_in: bool,
    /// Typst numbering is shown only when the heading carries it; unnumbered
    /// headings (a bibliography) take `plain` instead.
    plain: Option<&'static Level>,
}

const fn lvl() -> Level {
    Level {
        center: false,
        bold: false,
        italic: false,
        caps: false,
        small_caps: false,
        run_in: false,
        plain: None,
    }
}

const LEFT: Level = lvl();
const LEFT_BOLD: Level = Level {
    bold: true,
    ..lvl()
};
const LEFT_ITALIC: Level = Level {
    italic: true,
    ..lvl()
};
const LEFT_BOLD_ITALIC: Level = Level {
    bold: true,
    italic: true,
    ..lvl()
};
const LEFT_CAPS: Level = Level {
    caps: true,
    ..lvl()
};
const CENTER: Level = Level {
    center: true,
    ..lvl()
};
const CENTER_BOLD: Level = Level {
    center: true,
    bold: true,
    ..lvl()
};
const CENTER_ITALIC: Level = Level {
    center: true,
    italic: true,
    ..lvl()
};
const CENTER_CAPS: Level = Level {
    center: true,
    caps: true,
    ..lvl()
};
const RUN_IN_BOLD: Level = Level {
    run_in: true,
    bold: true,
    ..lvl()
};
const RUN_IN_BOLD_ITALIC: Level = Level {
    run_in: true,
    bold: true,
    italic: true,
    ..lvl()
};
const RUN_IN_ITALIC: Level = Level {
    run_in: true,
    italic: true,
    ..lvl()
};
const GOST_STRUCTURAL: Level = Level {
    center: true,
    caps: true,
    bold: true,
    ..lvl()
};
const GOST_SECTION: Level = Level {
    bold: true,
    plain: Some(&GOST_STRUCTURAL),
    ..lvl()
};
const IEEE_SECTION: Level = Level {
    center: true,
    small_caps: true,
    ..lvl()
};

fn levels(style_key: &str) -> Option<(&'static str, &'static [Level])> {
    Some(match style_key {
        "sbl" => (
            "SBL",
            &[CENTER_CAPS, CENTER_BOLD, CENTER, LEFT_BOLD_ITALIC, LEFT],
        ),
        "chicago-notes" => (
            "Chicago (Notes-Bibliography)",
            &[CENTER_BOLD, CENTER_ITALIC, LEFT_ITALIC],
        ),
        "chicago-author-date" => (
            "Chicago (Author-Date)",
            &[CENTER_BOLD, CENTER_ITALIC, LEFT_ITALIC],
        ),
        "turabian" => ("Turabian", &[CENTER_BOLD, CENTER, LEFT_ITALIC, LEFT]),
        "mla" => ("MLA", &[LEFT]),
        "apa" | "harvard" => (
            "APA",
            &[
                CENTER_BOLD,
                LEFT_BOLD,
                LEFT_BOLD_ITALIC,
                RUN_IN_BOLD,
                RUN_IN_BOLD_ITALIC,
            ],
        ),
        "asa" => ("ASA", &[LEFT_CAPS, LEFT_ITALIC, RUN_IN_ITALIC]),
        "ieee" => ("IEEE", &[IEEE_SECTION, LEFT_ITALIC, RUN_IN_ITALIC]),
        "gost-r-705" => ("GOST R 7.0.5", &[GOST_SECTION, LEFT_BOLD, LEFT_BOLD_ITALIC]),
        "vancouver" => ("Vancouver", &[LEFT_BOLD, LEFT_BOLD_ITALIC, LEFT_ITALIC]),
        _ => return None,
    })
}

fn size_divisor(level: usize) -> Option<&'static str> {
    match level {
        1 => Some("1.4"),
        2 => Some("1.2"),
        _ => None,
    }
}

fn styled(l: &Level, inner: &str) -> String {
    let mut out = inner.to_string();
    if l.caps {
        out = out.replace("#it.body", "#upper(it.body)");
    }
    if l.small_caps {
        out = format!("#smallcaps[{out}]");
    }
    match (l.bold, l.italic) {
        (true, true) => format!("#text(weight: \"bold\", style: \"italic\")[{out}]"),
        (true, false) => format!("#text(weight: \"bold\")[{out}]"),
        (false, true) => format!("#text(style: \"italic\")[{out}]"),
        (false, false) => out,
    }
}

fn block_rule(level: usize, l: &Level, gap: &str, indent: Option<&str>) -> String {
    let (above, below) = match gap {
        "ieee" => ("1.2em", "0.6em"),
        _ => ("par.leading", "par.leading"),
    };
    let size = size_divisor(level)
        .map(|k| format!("size: 1em / {k}, "))
        .unwrap_or_default();
    let body = styled(l, "#it.body");
    let shaped = if l.center {
        format!("#align(center)[{body}]")
    } else if let Some(i) = indent {
        format!("#h({i}){body}")
    } else {
        body
    };
    format!(
        "#show heading.where(level: {level}): it => context {{\n  \
         set text({size}weight: \"regular\")\n  \
         set par(first-line-indent: 0pt)\n  \
         block(width: 100%, above: {above}, below: {below})[\n    {shaped}\n  ]\n}}"
    )
}

fn gost_rule(level: usize, l: &Level, gap: &str) -> String {
    let plain = l.plain.expect("GOST sections carry a structural form");
    let numbered = styled(l, "#it.body");
    let unnumbered = styled(plain, "#upper(it.body)");
    let size = size_divisor(level)
        .map(|k| format!("size: 1em / {k}, "))
        .unwrap_or_default();
    let _ = gap;
    format!(
        "#show heading.where(level: {level}): it => context {{\n  \
         set text({size}weight: \"regular\")\n  \
         set par(first-line-indent: 0pt)\n  \
         block(width: 100%, above: par.leading, below: par.leading)[\n    \
         #if it.numbering == none [#align(center)[{unnumbered}]] else [#h(12.5mm){numbered}]\n  ]\n}}"
    )
}

fn run_in_rule(level: usize, l: &Level) -> String {
    let body = styled(l, "#it.body.");
    format!(
        "#show heading.where(level: {level}): it => {{ set text(weight: \"regular\"); [{body} ] }}"
    )
}

pub(crate) fn heading_styles(style_key: &str) -> String {
    if style_key == "latex" {
        return "\n// LaTeX-look heading spacing\n#show heading: set block(above: 1.4em, below: 1em)"
            .to_string();
    }
    let Some((name, spec)) = levels(style_key) else {
        return "\n// Default heading styles\n\
                #show heading.where(level: 1): it => block(width: 100%, above: 1em, below: 0.5em)[\n  \
                #set par(first-line-indent: 0pt)\n  \
                #align(center)[#text(weight: \"bold\")[#it.body]]\n]\n\
                #show heading.where(level: 2): it => block(width: 100%, above: 0.8em, below: 0.4em)[\n  \
                #set par(first-line-indent: 0pt)\n  \
                #text(weight: \"bold\")[#it.body]\n]"
            .to_string();
    };
    let gap = if style_key == "ieee" { "ieee" } else { "" };
    let mut out = format!("\n// {name} heading styles");

    let last = spec.last().expect("every style defines a level");
    for i in 0..5 {
        let l = spec.get(i).unwrap_or(last);
        let level = i + 1;
        out.push('\n');
        out.push_str(&if l.plain.is_some() {
            gost_rule(level, l, gap)
        } else if l.run_in {
            run_in_rule(level, l)
        } else {
            block_rule(level, l, gap, None)
        });
    }
    out.push_str(extras(style_key));
    out
}

/// Rules that belong with a style's headings: they are swapped together when
/// the style changes.
fn extras(style_key: &str) -> &'static str {
    match style_key {
        "mla" => "\n#show bibliography: it => { show heading: set align(center); it }",
        "sbl" | "turabian" | "chicago-notes" => {
            "\n#show footnote.entry: it => {\n  \
             set text(size: 10pt)\n  \
             set par(first-line-indent: 0.5in, leading: 0.65em, spacing: 0.65em, justify: false)\n  \
             context { numbering(it.note.numbering, ..counter(footnote).at(it.note.location())); [. ]; it.note.body }\n}\n\
             #show bibliography: set par(leading: 0.65em, spacing: 1.2em)"
        }
        _ => "",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_rule_resets_the_default_heading_size_and_weight() {
        for key in ["sbl", "apa", "mla", "ieee", "gost-r-705", "turabian"] {
            let rules = heading_styles(key);
            assert!(
                rules.contains("weight: \"regular\""),
                "{key} must reset heading weight"
            );
        }
    }

    #[test]
    fn run_in_levels_do_not_open_a_block() {
        let rules = heading_styles("apa");
        let l4 = rules
            .split("#show heading.where(level: 4)")
            .nth(1)
            .unwrap()
            .lines()
            .next()
            .unwrap();
        assert!(!l4.contains("block("));
    }
}
