# Security

## Supported versions

Only the latest version of Snippets gets security fixes.

| Version | Supported |
|---|---|
| 2.1.x | Yes |
| Older | No |

## Reporting a problem

Please do not open a public issue for a security problem before it is fixed.

1. Use GitHub's private vulnerability reporting: open the repository's Security tab and choose "Report a vulnerability".
2. If you cannot use GitHub, email <distro@trindle.org>.

This is a small project looked after by one person. You will get an answer, but not always the same day.

## How Snippets is put together

- It makes no network connections and runs no other program.
- It writes only inside the snippet folder, its backups folder and its own home. Names read from an imported archive are cut down to a plain file name, and nothing is unpacked onto the disk: archives are read in memory with limits on the number of files, each file, the total, and the inflated size.
- An imported file with shell or script variables is named, and written only after you say yes, because a real Espanso would run them.
- A save writes the new file whole or not at all, after keeping a copy of the old one that only you can read. If the copy cannot be kept, the save is refused.
- Before writing, Snippets reads the new text back and checks that no other snippet in the file changed.
- `expansions.json` holds plain snippets only: no variables, forms or scripts, and no control characters other than a new line or tab in the words.
- One window only, through a name on the session bus; a second start asks the first to come forward and leaves.
