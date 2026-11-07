# Changelog

Notable changes to Snippets.

## 2.1.0 (not yet released)

- Asks for the `expand` permission; once allowed, Trindle can type your plain snippets for you. The starter trigger is `:tm`.
- The ring icon has a menu to turn typing expansion off and on.
- The starter snippet is `:tm`, which types "Typed by Snippets".
- The buttons carry their own icons, so they no longer draw blank inside the plugin's box.
- One window only.
- A snippet file that is a link to another file is read-only: Snippets never writes through a link.
- Search marks the letters it matched.
- Copy says why when there is nothing to copy.
- Fixes from a code review:
  - Save and delete go by the snippet as you opened it, so an outside edit is reported, not overwritten.
  - Unsaved words are kept when you pick another snippet, press New or close the window.
  - Delete asks twice, and Put back undoes it.
  - A save is refused if its backup cannot be made; backups are 20 per file and private.
  - Import has size limits, runs off the window, and asks before bringing in shell or script variables.
  - Builds from a plain `cargo test`: the Trindle Toolkit is taken from its public repository at one pinned commit, so no private token or sibling folder is needed.
  - Backup copies of `a/b.yml` and `a_b.yml` no longer share one pool of twenty.
  - The home, config and data folders come from the shared trindle-common helpers.
  - Snippets stops with a message when `HOME` is not a full path, and `--expand` needs `on` or `off`.
  - A watched tree of exactly 256 folders says that some are not watched.
  - The long source files are split into smaller ones, with no change in what they do.
  - Comments survive deleting the only snippet and adding to an empty list; CRLF files keep CRLF.
  - Typing expansion leaves out snippets that match in special ways, repeated triggers and control characters.

## 2.0.0

- Snippets rewritten in Rust on the Trindle Toolkit, as a Trindle plugin (`org.trindle.Snippets`).
