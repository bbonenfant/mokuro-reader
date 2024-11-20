# Mokuro Reader/Editor

I'm an admirer and user of the Mokuro project.
One feature that was missing from the project was an ability to easily
correct and modify the output file(s).
So I decided to try to make such a tool, and this is the result.

## Feature Wishlist

- [x] Mokuro Editor
- [x] Reader Magnifying Glass
- [ ] Volume Ordering & Filtering
- [ ] Add "Bookmarks" & "Chapter Markers" to Volumes
- [ ] (Stretch) Anki Integration (with image cropping)

## Important Notes

* I am not affiliated with the mokuro project in any way -- I'm mostly just
  trying to build something useful to me.

* This app reads .mbz.zip files created from
  [this fork of the mokuro repo](https://github.com/bbonenfant/mokuro).
  These files contain both the manga image files and the OCR output.

### Browser Support

| Browser  |       Support       | Notes                                                                                                                            |
|----------|:-------------------:|----------------------------------------------------------------------------------------------------------------------------------|
| Firefox  |         Yes         | All features supported.                                                                                                          |
| Chromium |         Yes         | Drag-resizing is a bit broken for vertical textboxes. <br/> [On track to be fixed](https://issues.chromium.org/issues/363806067) |
| Safari   | Not<br/>Recommended | Reader should work fine, but editing text is wonky. <br/> Moving the cursor with arrow keys is confusing for vertical textboxes. |                                                                   

## Summary

This is a web app where you can upload, read, and modify your Mokuro
manga volumes (where the files are generated by
[my fork of mokuro](https://github.com/bbonenfant/mokuro).)

All files are local to your browser — absolutely nothing leaves your machine.
Your manga volumes are stored in your browser's IndexedDB system,
which means the content of your library is specific to your machine
and browser, i.e. your files are not shared between Chrome and Firefox nor
between your laptop and desktop.

When uploading volumes, you will be prompted to "Persist Your Storage".
This will protect your files from being deleted if your browser ever
needs to free up storage space. Your modified volumes can be exported
and downloaded as .mbz.zip files from the Home/Library page by clicking
the "Prepare Download" button and then the "Download" button.

In the Reader view,you can enable editing mode by pressing "E".
This mode allows you to modify the OCR output generated by Mokuro (the textboxes).
Functionality includes editing the text, resizing and moving the textboxes,
creating new textboxes, and adjusting the font size.

Most actions have a keyboard shortcut, and some have mouse-based equivalents.
Additionally, a "magnifying glass" can be enabled by right-clicking the reader.
When in the reader view, press "H" to display the help banner.

## Actions

### Reader Actions

| Action           | Key | Mouse                                                 |
|------------------|-----|-------------------------------------------------------|
| Toggle Help      | H   | Checking "Show Help" in the Settings bar.             |
| Next Page        | Z   | Clicking the vertical bar to the left of the reader.  | 
| Previous Page    | X   | Clicking the vertical bar to the right of the reader. |
| Toggle Editing   | E   | N/A                                                   |
| Toggle Sidebar   | S   | Clicking the sidebar on the left (if not hidden)      |
| Toggle Magnifier | N/A | Right Click on reader                                 |

### Editor Actions

| Action                  | Key                  | Mouse                                                                                                                                               |
|-------------------------|----------------------|-----------------------------------------------------------------------------------------------------------------------------------------------------|
| Move Textbox            | Arrow Keys           | Dragging textbox, when not editing text.                                                                                                            |
| Manually Resize Textbox | N/A                  | Drag bottom corner of textbox when focused, when not editing text.                                                                                  |
| Auto Resize Textbox     | "0" (zero)           | N/A                                                                                                                                                 |
| Create New Textbox      | N/A                  | Click and drag to select an area of the page. <br/> - Drag right-to-left for a vertical textbox <br/> - Drag left-to-right for a horizontal textbox |
| Delete Textbox          | Backspace            | N/A                                                                                                                                                 |
| Decrease Font Size      | "\-" (minus)         | N/A                                                                                                                                                 |
| Increase Font Size      | "\+" (plus)          | N/A                                                                                                                                                 |
| Toggle Text Opacity     | "\\" (forward slash) | N/A                                                                                                                                                 |
| Begin Editing Text      | "\`" (backtick)      | Double-click textbox.                                                                                                                               |
| End Editing Text        | Escape               | Clicking outside of textbox.                                                                                                                        |
| Select Next Textbox     | Tab                  | N/A                                                                                                                                                 |
