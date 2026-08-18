use std::collections::HashMap;
use std::path::Path;

/// Compile a Typst file to an HTML fragment for embedding in a website.
/// Footnotes become hover tooltips that respond to `data-theme="dark/light"`,
/// `.dark`/`.light` classes, and `prefers-color-scheme`.
pub fn export_for_web(input_path: &Path, output_path: &Path) -> Result<(), String> {
    let raw = run_pandoc(input_path)?;
    let fragment = build_fragment(&raw);
    std::fs::write(output_path, fragment).map_err(|e| e.to_string())
}

fn run_pandoc(input: &Path) -> Result<String, String> {
    let out = crate::git_sync::host_command("pandoc")
        .args([
            "-f",
            "typst",
            "-t",
            "html5",
            "--no-highlight",
            "--wrap=none",
            input.to_str().unwrap_or(""),
        ])
        .output()
        .map_err(|e| format!("Could not run pandoc: {e}"))?;
    if !out.status.success() {
        return Err(format!("pandoc: {}", String::from_utf8_lossy(&out.stderr)));
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

fn build_fragment(html: &str) -> String {
    let footnotes = collect_footnotes(html);
    let body = strip_footnote_section(html);
    let body = inline_tooltips(body, &footnotes);
    format!("{STYLE}\n{body}")
}

/// Returns map of footnote id (e.g. "fn1") → HTML text with back-link stripped.
fn collect_footnotes(html: &str) -> HashMap<String, String> {
    let mut map = HashMap::new();
    let mut pos = 0;
    while let Some(rel) = html[pos..].find("<li id=\"fn") {
        let abs = pos + rel;
        // Extract id value
        let id_start = abs + "<li id=\"".len();
        let Some(id_len) = html[id_start..].find('"') else {
            break;
        };
        let id = html[id_start..id_start + id_len].to_string();
        // Find closing </li>
        let Some(end_rel) = html[abs..].find("</li>") else {
            break;
        };
        let li = &html[abs..abs + end_rel + 5];
        map.insert(id, footnote_inner_text(li));
        pos = abs + 1;
    }
    map
}

fn footnote_inner_text(li: &str) -> String {
    // Strip "<li id="..."><p>" prefix and "</p></li>" suffix
    let after_tag = li.find('>').map(|i| i + 1).unwrap_or(0);
    let s = li[after_tag..].trim();
    let s = s.strip_prefix("<p>").unwrap_or(s);
    let s = s.strip_suffix("</p></li>").unwrap_or(s);
    let s = s.strip_suffix("</p>").unwrap_or(s);
    let s = s.trim();
    // Remove back-link: last <a ... class="footnote-back" ...>↩︎</a>
    strip_backlink(s).trim().to_string()
}

fn strip_backlink(s: &str) -> String {
    if let Some(a_pos) = s.rfind("<a ") {
        if s[a_pos..].contains("footnote-back") {
            let before = &s[..a_pos];
            let after_a = s[a_pos..]
                .find("</a>")
                .map(|i| a_pos + i + 4)
                .unwrap_or(s.len());
            return format!("{}{}", before.trim_end(), &s[after_a..]);
        }
    }
    s.to_string()
}

fn strip_footnote_section(html: &str) -> &str {
    for marker in &[
        "<section id=\"footnotes\"",
        "<section class=\"footnotes\"",
        "<div class=\"footnotes\"",
        "<div id=\"footnotes\"",
    ] {
        if let Some(pos) = html.find(marker) {
            return &html[..pos];
        }
    }
    html
}

fn inline_tooltips(html: &str, footnotes: &HashMap<String, String>) -> String {
    let mut out = String::with_capacity(html.len() + 512);
    let mut pos = 0;

    while let Some(rel) = html[pos..].find("<sup id=\"fnref") {
        let abs = pos + rel;
        out.push_str(&html[pos..abs]);

        // Find closing </sup>
        let Some(end_rel) = html[abs..].find("</sup>") else {
            out.push_str(&html[abs..]);
            return out;
        };
        let sup = &html[abs..abs + end_rel + 6];

        // Extract the display number (text inside the <a>)
        let num = link_text(sup).unwrap_or_else(|| "∗".to_string());

        // Derive footnote id: "fnref1" → "fn1" (handles "fnref1:1" duplicates too)
        let fn_id = fnref_to_fn_id(sup).unwrap_or_default();
        let note = footnotes.get(&fn_id).map(|s| s.as_str()).unwrap_or("");

        out.push_str(&format!(
            "<span class=\"zk-ref\"><sup>{num}</sup><span class=\"zk-note\">{note}</span></span>",
        ));
        pos = abs + end_rel + 6;
    }
    out.push_str(&html[pos..]);
    out
}

fn link_text(sup: &str) -> Option<String> {
    // Find the last > before </a> and take text up to </a>
    let a_end = sup.find("</a>")?;
    let before = &sup[..a_end];
    let last_close = before.rfind('>')?;
    Some(before[last_close + 1..].to_string())
}

fn fnref_to_fn_id(sup: &str) -> Option<String> {
    // <sup id="fnref1"> or <sup id="fnref1:1">
    let start = sup.find("id=\"")? + 4;
    let end = sup[start..].find('"')?;
    let raw = &sup[start..start + end]; // e.g. "fnref1" or "fnref1:1"
                                        // Strip ":N" duplicate suffix, replace "fnref" with "fn"
    let base = raw.split(':').next().unwrap_or(raw);
    Some(base.replacen("fnref", "fn", 1))
}

const STYLE: &str = r#"<style>
.zk-ref {
  position: relative;
  display: inline;
  cursor: help;
}
.zk-note {
  display: none;
  position: absolute;
  bottom: calc(100% + 6px);
  left: 50%;
  transform: translateX(-50%);
  width: max-content;
  max-width: min(320px, 80vw);
  padding: 8px 12px;
  border-radius: 6px;
  font-size: 0.875em;
  font-weight: normal;
  font-style: normal;
  line-height: 1.5;
  text-align: left;
  white-space: normal;
  z-index: 1000;
  pointer-events: none;
  background: #fff;
  color: #111;
  border: 1px solid #ddd;
  box-shadow: 0 4px 20px rgba(0,0,0,0.12);
}
.zk-ref:hover .zk-note { display: block; }
[data-theme="dark"] .zk-note,
.dark .zk-note,
.theme-dark .zk-note {
  background: #1e1e1e;
  color: #e0e0e0;
  border-color: #444;
  box-shadow: 0 4px 20px rgba(0,0,0,0.5);
}
[data-theme="light"] .zk-note,
.light .zk-note,
.theme-light .zk-note {
  background: #fff;
  color: #111;
  border-color: #ddd;
  box-shadow: 0 4px 20px rgba(0,0,0,0.12);
}
@media (prefers-color-scheme: dark) {
  .zk-note {
    background: #1e1e1e;
    color: #e0e0e0;
    border-color: #444;
    box-shadow: 0 4px 20px rgba(0,0,0,0.5);
  }
}
</style>"#;
