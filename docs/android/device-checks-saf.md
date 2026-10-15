# Device checks: open and save through the Storage Access Framework (WP 1.5/1.6)

For the owner, on the phone (SM-G985F, Android 13) and then the tablet. Branch `android-phase1`.
About 30 minutes. Each step says what should happen; anything else is a defect worth a screenshot
and the log.

## Before you start

1. Build and install: `packaging/android/deploy.sh arm64-v8a` (draft mode).
2. In a second terminal, keep the log open: `adb logcat -s PhotoCraft`. No line should contain
   `panic`.
3. Have in Downloads: a JPEG, a layered PSD (a big one, 100 MB or more, helps with step 12), and
   a PNG.

## Open

1. **Open a JPEG.** File › Open. The system picker appears and lists every file, PSDs included.
   Pick the JPEG. A tab named after the file opens (for example `IMG_0001.jpg`) and shows the image;
   the status bar says "Opened …".
2. **Several at once.** File › Open, long-press to select two or three images, then Select. Each
   opens in its own tab, in name order.
3. **A PSD.** File › Open the PSD. Its layers are in the Layers panel. With a big PSD, the window
   keeps responding while "Opening …" shows progress.
4. **Cancel.** File › Open, then Back out of the picker. Nothing opens and no error appears.

## Save

5. **Save back (FR-FIO-05).** In the PSD, paint a stroke, then File › Save (or Ctrl+S on a
   keyboard). The status bar shows "Saving …", then "Saved <name>.psd". In the Files app, the
   PSD's modified time is now. Close the tab and reopen it from File › Open Recent: the stroke is
   there.
6. **Save As .pcraft (FR-FIO-06).** File › Save As. The system "Save" screen offers `<name>.psd`.
   Change the name to `<name>.pcraft` and save. The status bar says "Saved <name>.pcraft" and the
   tab is renamed. Close it, reopen it from Open Recent: it is identical.
7. **Name taken.** Save As again to the same name in the same folder. Android keeps both and calls
   the new one `<name> (1).pcraft`; the tab shows that name.
8. **Export a PNG (FR-FIO-08).** File › Export › Export As…, choose PNG, Export. The Save screen
   offers `<name>.png`. Save it in Pictures. It opens in the Files app and in Photos or Gallery
   with the right picture. (Photos can take a moment to list it.)
9. **Cancel a save.** File › Save As, then Back out. No error; the document is unchanged and still
   marked unsaved if it was.

## Open Recent and permissions (FR-FIO-11)

10. **After a restart.** Open a document and let it finish opening (Open Recent is saved with the
    preferences right after), swipe PhotoCraft away from the recent apps, start it again. File › Open Recent lists
    the document; choosing it opens it without a picker. Repeat after rebooting the phone.
11. **A deleted file.** Open a file, close its tab, delete the file in the Files app. File › Open
    Recent › that file: a notice says "Couldn't open <name>: it isn't there any more (moved or
    deleted, or its storage was removed)." Nothing crashes.
12. **A withdrawn permission.** Open a file from another app's storage (Google Drive, or a
    third-party file manager that appears in the picker's side menu), then uninstall that app or
    clear its data in Settings › Apps.
    - From Open Recent: a notice says PhotoCraft no longer has permission (or that the file isn't
      there any more).
    - With the document still open: paint, then File › Save. The message says PhotoCraft can't
      change it, and the Save screen opens by itself to save it somewhere else. Saving there works.
13. **A read-only file.** Open a file from a place that only allows reading (for example a
    view-only file in Google Drive). Edit, File › Save: the message says the file can't be changed
    where it is, and the Save screen opens by itself.
14. **Clear Recent Files.** File › Open Recent › Clear Recent Files. Optional, with adb:
    `adb shell dumpsys activity permissions` lists the URI permissions apps hold; PhotoCraft's
    lasting grants drop to the documents still open.

## Never lose the document (FR-FIO-07)

15. **Killed while saving.** Open the big PSD, paint, File › Save, and while the status bar shows
    "Saving …" run `adb shell am force-stop local.photocraft.spike`. Start PhotoCraft again.
    - If the stop came while the file was being written, PhotoCraft reopens the document
      ("Recovering <name>"), marked unsaved. File › Save finishes the save; the PSD then opens
      fine in PhotoCraft (and on the desktop).
    - If it came while encoding (earlier), nothing is offered, and the original PSD is untouched:
      it opens exactly as before.
    Either way, the PSD is never left empty or cut short without the document being offered back.
16. **Folders.** File › Import › Video Frames to Layers… (it asks for a folder): a message says
    choosing a folder isn't available on Android yet (WP 1.7). Nothing crashes.

## Report back

For each step: pass or fail, a screenshot of anything wrong, and the `adb logcat -s PhotoCraft`
lines around it.
