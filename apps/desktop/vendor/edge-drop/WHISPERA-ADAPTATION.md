# Whispera Library

Adapted renderer from Edge Drop, commit 469fd0d56d8d4073ed5a6fbcab36953db8203cae.
Original source: https://github.com/Deepender25/Edge-Drop
Original license: Apache-2.0, retained in LICENSE.

Changes: remove color/emoji filters, remove footer count/community footer,
add video filter, preserve renderer motion, bridge storage/clipboard/file drag
to Whispera's existing Tauri process. No Electron process is started.

Build: npm install --ignore-scripts, then run the desktop build.
The browser adapter is only a preview. Native clipboard and drag require Tauri.
Update links target Whispera; automatic update installation is not configured.
Personal screenshot fixtures are excluded from the distributed renderer.
