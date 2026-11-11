# Snippets help

Snippets keeps your text snippets in one searchable list. A snippet is a short code you type, such as `:tm`, and the words it stands for. You add, change and delete snippets with a form, so you never edit the files by hand.

## Starting it

Open Snippets from Apps in Trindle, or from its icon in the ring. Only one window opens: starting it again brings the first one to the front.

A fresh install has one snippet, `:tm`, which turns into "Typed by Snippets".

## Making typing expansion work

1. When Trindle asks, allow `expand`. It is off until you allow it.
2. In any program, type a trigger such as `:tm`. Trindle swaps it for the words.

Without `expand`, Snippets still lists, edits and copies your snippets. Nothing is typed for you.

The Snippets icon in the ring has a menu with **Turn typing expansion off** and **Turn typing expansion on**. Switching it off keeps all your snippets and stops them being typed. Switching it on brings them back.

Only plain snippets expand: no variables, forms or scripts. A trigger must be three to 32 characters with no spaces. The replacement must not be empty and can be up to 4000 bytes. Up to 2000 snippets are offered to Trindle. Snippets that do not fit are still in the list, but they do not expand.

## Using the window

- **Search** at the top finds triggers and text, and marks the letters it matched.
- **Plus** adds a new snippet. Give it one or more triggers (separate several with a comma) and the replacement words, then press **Save**.
- Pick a snippet in the list to change it. **Save** keeps your changes. **Put back as it was** drops them.
- **Delete** asks twice: press it, then press it again. After a delete, **Put back** undoes it.
- **Copy** beside the replacement puts the words on the clipboard, to paste anywhere.
- **Import** takes `.yml`, `.yaml` or `.tar.gz` files. They are added beside your own files, never over them. **Export** saves every snippet file as one `my-snippets.tar.gz`. **Read the snippet files again** (the reload button) picks up changes made elsewhere.
- If you pick another snippet or close the window with words not saved, they are kept and come back next time as a new snippet.

## Settings

Snippets has no settings page. These are the permissions Trindle shows, and the two switches on each snippet.

| Setting | What it changes |
|---|---|
| Permission `files` (required, allowed on install) | Lets Snippets read and write your snippet files and its backups. Without it, nothing works. |
| Permission `expand` (off until you allow it) | Lets Trindle type your plain snippets when you type their triggers. |
| Ring menu: Turn typing expansion on or off | Stops or restarts typing without touching your snippets. |
| Whole words only (each snippet) | The trigger only fires when it stands as a whole word, not in the middle of another word. |
| Match the case I type (each snippet) | Typing the trigger in capitals gives the words in capitals. |

## Where things are kept

| What | Where |
|---|---|
| Snippet files | `~/.config/espanso/match/*.yml`, or `~/.espanso/match/` where that already holds them. As a Trindle plugin, `~` is the plugin's own home. |
| Backups | `~/.local/share/snippets/backups`, 20 copies of each file |
| List for typing expansion | `expansions.json` in the plugin's home |

Snippets reads and writes Espanso's file format, so your snippets move between Snippets and Espanso. A save changes only the lines you edited. Comments, order and blank lines stay. A copy of the file is kept first, and the save is refused if that copy cannot be made.

## When something goes wrong

| What you see | What it means and what to do |
|---|---|
| Nothing is typed when you type a trigger | Check that `expand` is allowed, that the ring menu does not say expansion is off, and that the snippet is plain (no variables, forms or scripts) and its trigger is three to 32 characters with no spaces. |
| "Give it at least one trigger." / "A trigger cannot hold spaces. Try :addr." / "That trigger is too long." | Fix the trigger box. Triggers have no spaces. The editor takes up to 100 characters, but only 32 or fewer expand. |
| "Give it something to turn into." / "That is too long for a snippet." | The replacement is empty, or past the limit of 200,000 characters. Typing expansion takes up to 4000 bytes. |
| "Nothing was saved, because a copy of the file could not be kept" | Snippets could not make its backup, so it did not touch your file. Make sure there is room and the backups folder can be written to, then save again. |
| "That snippet has changed in its file since it was shown. Reload." / "That snippet is not there any more. Reload." | Another program changed the file. Press the reload button, then make your change again. |
| "That snippet was removed from its file. Your changes are kept; Save adds it again." | The snippet was deleted elsewhere. Press **Save** to add it back, or **Put back as it was** to drop your changes. |
| "Snippets could not save this without changing other snippets, so nothing was written." | The file is written in a way Snippets will not risk rewriting. Your file is untouched. Edit that snippet by hand. |
| "This snippet uses variables, forms or scripts, which Snippets shows but does not edit." (and similar) | Such snippets are shown read-only, as they are in the file, and never rewritten. Change them in the file itself. |
| "This file is a link to somewhere else, so it is read-only here." | The file in the snippet folder is a link to a file elsewhere. Snippets lists it but never writes through a link. Change the real file, or replace the link with a copy. |
| "This came from an installed package, so it is read-only here." | Snippets from an Espanso package cannot be changed here. Copy the words into a new snippet instead. |
| "Typing expansion could not be brought up to date" | Snippets could not write its list for Trindle. Your snippets are safe. Make sure the plugin's home has room, then save again or press reload. |
| "Changes made to the snippet files by other programs may not be noticed. Press the reload button to read them." | Snippets could not watch the folder. Press the reload button when you have changed files elsewhere. |
| "Choose a .yml, .yaml or .tar.gz file." / "No snippet files were found in it." / "That file is too large to be snippets." / "That archive holds too many files." | The import was refused. Check the file. A file can be up to 8 MB, and an archive up to 500 files. |
| An import that has shell or script variables waits for a yes | Those variables run programs in Espanso. Snippets lists the files and only brings them in when you say yes. |
| "snippets: there is no screen to show a window on" (from a terminal) | Snippets was started with no display. Start it from Trindle. |
| "snippets: HOME is not set to a full path" (from a terminal) | Snippets was started with no home folder. Start it from Trindle. |

## Privacy

- Snippets cannot see your keys, your screen or other programs. It only writes a plain list of triggers and words. Trindle does the watching and typing, and only once you allow `expand`.
- Snippets makes no network connections and runs no other program.
- It writes only to the snippet folder, its backups folder and its own home. It reads only the files you choose to import.
- Your snippets are plain text files that anyone with access to your account can read. Do not keep passwords in snippets.
- The words in a snippet are typed into whatever program has focus when you type the trigger, so keep triggers distinctive.

More detail: [README.md](../README.md), [SECURITY.md](../SECURITY.md), [TRIGGER-DOOR.md](TRIGGER-DOOR.md).
