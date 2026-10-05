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

settings-compilation-title = Preview
settings-compile-delay-title = Pause before the preview updates
settings-compile-delay-subtitle = How long Zerkalo waits after you stop typing, in thousandths of a second (only when the preview updates by itself)
settings-compile-mode-auto = By itself
settings-compile-mode-manual = When I ask
settings-compile-trigger-title = When the preview updates
settings-compile-trigger-subtitle = By itself: a moment after you pause · When I ask: when you save, press the preview button, or use Ctrl+Shift+P

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
settings-tab-width-subtitle = How many spaces the Tab key adds
settings-show-whitespace-title = Show whitespace
settings-spacing-compact = Compact (0 px)
settings-spacing-normal = Normal (2 px)
settings-spacing-spacious = Spacious (6 px)
settings-line-spacing-title = Line spacing
settings-line-spacing-subtitle = Extra pixels above and below each line
settings-typewriter-title = Typewriter scrolling
settings-typewriter-subtitle = Keep the cursor vertically centred as you type
settings-high-contrast-title = High contrast
settings-high-contrast-subtitle = Stronger text and outlines in the editor, easier on tired eyes
settings-word-count-goal-title = Word count goal
settings-word-count-goal-subtitle = A gentle progress ring in the status bar. Leave at 0 to hide it.

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
settings-custom-csl-title = Your own citation style
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
settings-simultaneous-imports-title = Files to import at once
settings-simultaneous-imports-subtitle = When you import a whole folder, how many documents Zerkalo works on together

## Keyboard Shortcuts

settings-keyboard-shortcuts-title = Keyboard Shortcuts
settings-shortcut-bindings-title = Shortcut bindings
settings-shortcut-bindings-subtitle = Customize any shortcut by editing a text file
settings-open-file-button = Open File
settings-open-file-failed-heading = Couldn't open that file
settings-open-file-failed-body = You can open it yourself at:
    { $path }

## Backup & Sync

settings-backup-sync-title = Backup & Sync
settings-backup-sync-description = Sign in with GitHub to keep a private online copy of your writing.
settings-account-title = Account
settings-connected = Connected
settings-not-connected = Not connected
settings-connected-as = Connected as { $username }
settings-reconnect-button = Reconnect
settings-signin-github-button = Sign in with GitHub
settings-disconnect-button = Disconnect
settings-backup-locations-title = Backup locations
settings-backup-locations-subtitle = Where Zerkalo keeps your online backups

## Setup

settings-setup-title = Setup
settings-setup-wizard-title = Setup wizard
settings-setup-wizard-subtitle = Go through the friendly first-time setup again
settings-run-button = Run…

## Pages

settings-page-general = General
settings-page-editor = Editor
settings-page-extras = References & Spelling

## Save-time validation notices

settings-work-folder-unusable-heading = Zerkalo can't use that work folder
settings-folder-create-failed-body = Zerkalo couldn't make the folder { $path }. Details: { $error }
settings-output-folder-unusable-heading = Zerkalo can't use that output folder
settings-bib-file-label = bibliography file
settings-custom-csl-file-label = citation style file
settings-skrizhal-file-label = Skrizhal file
settings-file-not-found-heading = Zerkalo can't find the { $label }
settings-file-not-found-body = There's no file at { $path }. Clear the box, or choose a different file.
settings-save-failed-heading = Couldn't save your settings
