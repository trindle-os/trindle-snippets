# How typing expansion works between Snippets and Trindle

Snippets cannot watch typing or type into other programs from its box, and must not. So Trindle does both and Snippets only supplies the list.

## What Snippets does
It writes `expansions.json` in its own home (`~/.local/share/trindle/plugins/org.trindle.Snippets/home/expansions.json`) whenever the list is read or changed, as a whole plain file (written to a temp name, then renamed):

    {"version":1,"snippets":[{"trigger":":tm","replace":"text","word":false,"case":false}]}

Only plain snippets go in: no variables, forms or scripts, trigger 3 to 32 characters with no spaces or control characters, replacement non-empty, at most 4000 bytes, with no control characters (no line breaks or tabs), at most 2000 entries. `word` means only when the trigger starts a word; `case` means copy the capitals of what was typed. The starter snippet is `:tm`.

## What Trindle does
1. A permission, for example `expand` ("expand your short codes as you type"), off until allowed, with a warning: Trindle watches what you type for the codes and types the words for you.
2. The compositor reads that file without following links, treats it as hostile (same limits again, plain text only, no key presses, no Enter or Tab), and keeps a short rolling buffer of letters typed. It never stores or sends the buffer anywhere, empties it on focus change and in password fields, and does nothing while the screen is locked or a question is showing.
3. On a match it removes the trigger (backspaces) and types the words with its own made-up keyboard.
4. Settings lists every active trigger with the plugin's name.
