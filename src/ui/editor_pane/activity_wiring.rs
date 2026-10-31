//! Feeds the private writing record (`crate::activity`): what is typed, and
//! what is pasted in.

use super::*;

pub(super) fn wire(tab: &TabContext) {
    let path = tab.path.clone();
    tab.buffer.connect_insert_text(move |_, _, text| {
        crate::activity::inserted(&path, text);
    });
    tab.view
        .connect_paste_clipboard(|_| crate::activity::paste_starting());
    tab.buffer
        .connect_paste_done(|_, _| crate::activity::paste_finished());
}
