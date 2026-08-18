# Settings dialog (src/ui/settings_dialog.rs) — reference locale (English).
# IDs are grouped to match the dialog's own PreferencesGroup layout.

## Window chrome

settings-window-title = Settings
settings-cancel = Cancel
settings-save = Save

## Folders

settings-folders-title = Folders
settings-work-folder-title = Work folder
settings-output-folder-title = Output folder
settings-browse-folder-tooltip = Browse for a folder
settings-browse-work-folder-a11y = Browse for a work folder
settings-browse-output-folder-a11y = Browse for an output folder

## Compilation

settings-compilation-title = Compilation
settings-compile-delay-title = Compile delay
settings-compile-delay-subtitle = How long to wait after you stop typing before updating the preview, in milliseconds (Auto mode only)
settings-compile-mode-auto = Auto
settings-compile-mode-on-save = On Save
settings-compile-mode-manual = Manual
settings-compile-trigger-title = Compile trigger
settings-compile-trigger-subtitle = Auto: after each keystroke · On Save: Ctrl+S only · Manual: Ctrl+Shift+P only

## Appearance

settings-appearance-title = Appearance
settings-theme-system = System
settings-theme-light = Light
settings-theme-dark = Dark
settings-color-scheme-title = Color scheme

## Editor

settings-editor-title = Editor
settings-editor-font-title = Editor font
settings-editor-font-subtitle = Family and size
settings-tab-width-title = Tab width
settings-tab-width-subtitle = Spaces
settings-word-wrap-title = Word wrap
settings-show-whitespace-title = Show whitespace
settings-spacing-compact = Compact (0 px)
settings-spacing-normal = Normal (2 px)
settings-spacing-spacious = Spacious (6 px)
settings-line-spacing-title = Line spacing
settings-line-spacing-subtitle = Extra pixels above and below each line
settings-typewriter-title = Typewriter scrolling
settings-typewriter-subtitle = Keep the cursor vertically centred as you type
settings-high-contrast-title = High contrast mode
settings-high-contrast-subtitle = Add extra CSS contrast to the editor and UI
settings-word-count-goal-title = Word count goal
settings-word-count-goal-subtitle = Show progress bar in status bar (0 = disabled)

## Document Fonts

settings-doc-fonts-title = Document Fonts
settings-doc-fonts-description = Used by new documents and template previews until a document picks its own.
settings-sans-serif-title = Sans-serif
settings-serif-title = Serif
settings-available-fonts-title = Available fonts
settings-available-fonts-subtitle = Enable or disable fonts Zerkalo can use
settings-manage-button = Manage…

## Bibliography

settings-bibliography-title = Bibliography
settings-bib-file-title = Bib file or Kartoteka vault
settings-browse-bib-tooltip = Browse for a .bib/.yaml file
settings-browse-bib-a11y = Browse for a bibliography file
settings-browse-vault-tooltip = Browse for a Kartoteka vault folder
settings-bibliography-description = A .bib/.yaml file — including a library exported from Zotero, Mendeley, or any other reference manager as BibTeX — or a Kartoteka vault folder for live citation autocomplete as you edit the vault.
settings-custom-csl-title = Custom CSL file
settings-browse-csl-tooltip = Browse for a .csl file
settings-browse-csl-a11y = Browse for a CSL style file
settings-csl-filter-name = CSL files (*.csl)

## CV Elements

settings-cv-elements-title = CV Elements
settings-cv-elements-description = Used in CV mode instead of the bibliography above — a Skrizhal YAML file of jobs, degrees, awards, etc.
settings-skrizhal-file-title = Skrizhal file
settings-browse-skrizhal-tooltip = Browse for a Skrizhal file
settings-yaml-filter-name = YAML files (*.yaml, *.yml)

## Spell Check

settings-spell-check-title = Spell Check
settings-enable-spell-check-title = Enable spell check
settings-remove-language-tooltip = Remove this language
settings-add-language-title = Add language
settings-add-button = Add

## Advanced

settings-advanced-title = Advanced
settings-simultaneous-imports-title = Simultaneous imports
settings-simultaneous-imports-subtitle = How many documents Import Folder converts at once

## Keyboard Shortcuts

settings-keyboard-shortcuts-title = Keyboard Shortcuts
settings-shortcut-bindings-title = Shortcut bindings
settings-shortcut-bindings-subtitle = Customize any shortcut by editing a text file
settings-open-file-button = Open File
settings-open-file-failed-heading = Couldn't open the file
settings-open-file-failed-body = Edit it by hand at:
    { $path }

## Backup & Sync

settings-backup-sync-title = Backup & Sync
settings-backup-sync-description = Sign in with GitHub to back up your work online when you sync.
settings-account-title = Account
settings-connected = Connected
settings-not-connected = Not connected
settings-connected-as = Connected as { $username }
settings-reconnect-button = Reconnect
settings-signin-github-button = Sign in with GitHub
settings-disconnect-button = Disconnect
settings-backup-locations-title = Backup locations
settings-backup-locations-subtitle = Where saved versions get sent when you sync

## Setup

settings-setup-title = Setup
settings-setup-wizard-title = Setup wizard
settings-setup-wizard-subtitle = Re-run the guided first-time setup
settings-run-button = Run…

## Pages

settings-page-general = General
settings-page-editor = Editor
settings-page-extras = References & Spelling

## Save-time validation notices

settings-work-folder-unusable-heading = Work folder isn't usable
settings-folder-create-failed-body = { $path } could not be created: { $error }
settings-output-folder-unusable-heading = Output folder isn't usable
settings-bib-file-label = Bib file
settings-custom-csl-file-label = Custom CSL file
settings-skrizhal-file-label = Skrizhal file
settings-file-not-found-heading = { $label } not found
settings-file-not-found-body = { $path } doesn't exist. Clear the field or pick another file.
settings-save-failed-heading = Failed to save settings
