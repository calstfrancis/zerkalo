use std::path::Path;

use crate::error::Result;

const MAIN_TYP_TEMPLATE: &str = "\
#set document(title: \"My Document\", author: \"\")
#set page(paper: \"a4\", margin: (x: 2.5cm, y: 2.5cm))
#set text(size: 11pt)
#set par(justify: true)

= Introduction

Your document begins here.
";

const GITIGNORE: &str = "*.pdf\n*.png\n.zerkalo/cache/\n";

pub fn init_project(path: &Path) -> Result<()> {
    std::fs::create_dir_all(path)?;
    std::fs::create_dir_all(path.join(".zerkalo"))?;

    let main_typ = path.join("main.typ");
    if !main_typ.exists() {
        std::fs::write(&main_typ, MAIN_TYP_TEMPLATE)?;
    }

    let gitignore = path.join(".gitignore");
    if !gitignore.exists() {
        std::fs::write(&gitignore, GITIGNORE)?;
    }

    // Init git repo only if there isn't one already
    if git2::Repository::open(path).is_err() {
        git2::Repository::init(path)?;
    }

    Ok(())
}
