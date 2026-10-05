#!/bin/sh
# Builds Text as a Trindle plugin (plugin interface 1).
#
#   packaging/trindle-plugin.sh [OUT_DIR]
#
# OUT_DIR (default dist/trindle-plugin) gets:
#   org.trindle.Text/                  the plugin folder, ready for `trindle-plugin install`
#   org.trindle.Text.tar.zst           the same folder, packed
#   org.trindle.Text.description.txt   one paragraph for the Store
#   org.trindle.Text.svg               the Store's icon
#
# Inside Trindle the app is called Text and never names the company, so the
# build works on a renamed copy of the source; the working tree is untouched.
# Needs node, npm, zstd and trindle-plugin; downloads Electron once, through
# electron-builder's own cache.
set -eu

ID=org.trindle.Text
ROOT=$(cd "$(dirname "$0")/.." && pwd)
OUT=$(mkdir -p "${1:-$ROOT/dist/trindle-plugin}" && cd "${1:-$ROOT/dist/trindle-plugin}" && pwd)
WORK=$ROOT/dist/trindle-plugin-work
STAGE=$WORK/stage
PLUGIN=$OUT/$ID
TRIPLET=$(gcc -dumpmachine 2>/dev/null || echo x86_64-linux-gnu)
LIBDIR=/usr/lib/$TRIPLET
VERSION=$(node -p "require('$ROOT/package.json').version")

say() { printf '%s\n' "$*"; }

# 1. A renamed copy of the app. Every name of the company goes, the bridge's
#    identifier and the data folder's name included.
rm -rf "$WORK"
mkdir -p "$STAGE"
cp -a "$ROOT/src" "$ROOT/assets" "$ROOT/LICENSE" "$ROOT/package.json" "$ROOT/package-lock.json" "$STAGE/"
cp -al "$ROOT/node_modules" "$STAGE/node_modules"
find "$STAGE/src" "$STAGE/assets" -type f \( -name '*.js' -o -name '*.html' -o -name '*.css' -o -name '*.svg' \) \
  -exec sed -i -e 's/Lightmorphic Text/Text/g' -e 's/lightmorphicText/textApp/g' -e 's/lightmorphic-text/text/g' -e 's/Lightmorphic //g' {} +
node - "$STAGE/package.json" "$ID" <<'EOF'
const fs = require('node:fs');
const [file, id] = process.argv.slice(2);
const p = JSON.parse(fs.readFileSync(file, 'utf8'));
p.name = 'text';
p.productName = 'Text';
p.author = 'Text';
p.homepage = undefined;
p.repository = undefined;
p.desktopName = undefined;
p.build = {
  appId: id,
  executableName: 'text',
  electronLanguages: ['en-GB', 'en-US'],
  files: ['src/**/*', 'assets/icon.png'],
  directories: { output: 'out' },
  linux: { target: 'dir' },
};
fs.writeFileSync(file, JSON.stringify(p, null, 2) + '\n');
EOF
if grep -rIli lightmorphic "$STAGE/src" "$STAGE/assets" "$STAGE/package.json"; then
  say "The renamed copy still names the company (files above)."; exit 1
fi

# 2. Electron with the app inside it.
(cd "$STAGE" && npx --no-install electron-builder --linux dir --publish never >"$WORK/builder.log" 2>&1) || {
  tail -20 "$WORK/builder.log"; exit 1; }
APP=$STAGE/out/linux-unpacked
# The setuid sandbox helper cannot work in a box; the box is the sandbox.
rm -f "$APP/chrome-sandbox"

# 3. The plugin folder.
rm -rf "$PLUGIN" "$OUT/$ID".*
mkdir -p "$PLUGIN/bin" "$PLUGIN/lib" "$PLUGIN/share/glib-2.0/schemas"
cp -a "$APP" "$PLUGIN/electron"
cp "$ROOT/LICENSE" "$PLUGIN/LICENSE"
cp "$ROOT/packaging/trindle/text-symbolic.svg" "$PLUGIN/"
cp "$ROOT/packaging/trindle/description.txt" "$OUT/$ID.description.txt"
cp "$STAGE/assets/icon.svg" "$OUT/$ID.svg"

# Every library Electron needs, except the C library and the graphics
# driver's own, which must be the computer's. The launcher uses one only when
# the computer lacks it, so a bundled copy never stands in for the system's.
ldd "$APP/text" | awk '/=> \//{print $3}' | while read -r lib; do
  case "${lib##*/}" in
    libc.so*|libm.so*|libdl.so*|libpthread.so*|librt.so*|libresolv.so*|ld-linux*|libgbm.so*|libdrm.so*) continue ;;
  esac
  case "$lib" in "$APP"/*) continue ;; esac
  cp -L "$lib" "$PLUGIN/lib/"
done
# NSS loads its own modules by name at run time.
if [ -f "$PLUGIN/lib/libnss3.so" ] && [ -d "$LIBDIR/nss" ]; then
  cp -L "$LIBDIR"/nss/*.so "$PLUGIN/lib/"
fi
# GTK's file chooser stops the program without its settings schemas.
for s in /usr/share/glib-2.0/schemas/org.gtk.Settings.*.gschema.xml; do cp "$s" "$PLUGIN/share/glib-2.0/schemas/"; done
glib-compile-schemas "$PLUGIN/share/glib-2.0/schemas"

cat >"$PLUGIN/bin/text" <<EOF
#!/bin/sh
# Starts Text in its box. Only the libraries this computer lacks are taken
# from the plugin (all of them with TEXT_ALL_LIBS=1, for testing).
app=\$(cd "\$(dirname "\$0")/.." && pwd)
libs=\$(mktemp -d)
for f in "\$app"/lib/*; do
  n=\${f##*/}
  if [ -n "\${TEXT_ALL_LIBS:-}" ] || { [ ! -e "$LIBDIR/\$n" ] && [ ! -e "/usr/lib/\$n" ]; }; then
    ln -s "\$f" "\$libs/\$n"
  fi
done
export LD_LIBRARY_PATH="\$libs"
if [ -n "\${TEXT_ALL_LIBS:-}" ] || [ ! -e /usr/share/glib-2.0/schemas/org.gtk.Settings.FileChooser.gschema.xml ]; then
  export GSETTINGS_SCHEMA_DIR="\$app/share/glib-2.0/schemas"
fi
# The file chooser matches the app: dark.
export GTK_THEME=Adwaita:dark
export TEXT_PLUGIN=1
exec "\$app/electron/text" --no-sandbox --ozone-platform=wayland --enable-features=WaylandWindowDecorations "\$@"
EOF
chmod 755 "$PLUGIN/bin/text"

cat >"$PLUGIN/plugin.ini" <<EOF
[Plugin]
Interface=1
Id=$ID
Name=Text
Version=$VERSION
Permissions=files

[Ring]
Icon=text-symbolic.svg
Label=Text
Run=bin/text
EOF

# 4. Nothing in it may name the company, the app's own archive included.
if grep -rlia lightmorphic "$PLUGIN" "$OUT/$ID.description.txt" "$OUT/$ID.svg"; then
  say "The plugin still names the company (files above)."; exit 1
fi

# 5. The validator has the last word.
trindle-plugin check "$PLUGIN"
tar -C "$OUT" -cf - "$ID" | zstd -19 -T0 -q -o "$OUT/$ID.tar.zst"
say "$(du -sh "$PLUGIN" | cut -f1) unpacked, $(du -h "$OUT/$ID.tar.zst" | cut -f1) packed: $OUT"
