//! Talking to a running Zotero, only to ask it which works you pick.
//!
//! Zotero with the Better BibTeX add-on listens on this computer (port 23119)
//! and can open its own "pick a citation" window, answering with the keys you
//! chose ("cite as you write"). Nothing leaves your machine, and nothing here
//! reads or changes your Zotero library.

use std::time::Duration;

const CAYW: &str = "http://127.0.0.1:23119/better-bibtex/cayw";

fn client(timeout: Duration) -> Result<reqwest::blocking::Client, String> {
    reqwest::blocking::Client::builder()
        .timeout(timeout)
        // Always local: never through a proxy.
        .no_proxy()
        .build()
        .map_err(|e| e.to_string())
}

/// Whether Zotero is running with Better BibTeX and ready to take a request.
/// Quick; call it off the main thread.
pub fn is_available() -> bool {
    client(Duration::from_millis(700))
        .and_then(|c| {
            c.get(CAYW)
                .query(&[("probe", "true")])
                .send()
                .and_then(|r| r.text())
                .map_err(|e| e.to_string())
        })
        .is_ok_and(|body| body.trim() == "ready")
}

/// Opens Zotero's own picker and waits (as long as you take) for your choice.
/// Blocks, so call it off the main thread.
pub fn pick() -> Result<Vec<String>, String> {
    let reply = client(Duration::from_secs(600))?
        .get(CAYW)
        .query(&[("format", "pandoc"), ("brackets", "false")])
        .send()
        .map_err(|_| {
            "Zotero isn't answering. Open Zotero (with Better BibTeX installed) and try again."
                .to_string()
        })?;
    if !reply.status().is_success() {
        return Err(
            "Zotero answered, but not as Better BibTeX. Install the Better BibTeX add-on.".into(),
        );
    }
    let body = reply.text().map_err(|e| e.to_string())?;
    Ok(keys_from_reply(&body))
}

/// The keys in Zotero's answer (`@smith2019; @doe2020`).
pub fn keys_from_reply(body: &str) -> Vec<String> {
    crate::citation_keys::find_uses(body)
        .into_iter()
        .map(|u| u.key)
        .collect()
}

/// What to put in the document for the picked keys: `@a @b`.
pub fn insert_text(keys: &[String]) -> String {
    keys.iter()
        .map(|k| format!("@{k}"))
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zoteros_answer_becomes_keys() {
        assert_eq!(
            keys_from_reply("@smith2019; @doe2020\n"),
            ["smith2019", "doe2020"]
        );
        assert_eq!(keys_from_reply("@žižek2008"), ["žižek2008"]);
        assert!(keys_from_reply("").is_empty());
    }

    #[test]
    fn the_inserted_text_is_plain_citations() {
        assert_eq!(insert_text(&["a".into(), "b".into()]), "@a @b");
        assert_eq!(insert_text(&[]), "");
    }
}
