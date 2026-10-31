# Voice Memos Sync (typed Kennel extension)

This is a thin Rust/WASM extension using the typed filesystem subscription
and scoped native copy API from Kennel PR #15.

### How it works

1. iPhone Voice Memos synchronizes recordings to the native macOS Voice Memos app through iCloud.
2. Kennel scans and reconciles the Voice Memos recordings directory every minute.
3. Only matching .m4a files that have stopped changing are delivered as typed FsEvent<M4aFile>.
4. The extension requests a native, asynchronous copy into the selected Drive account's Voice Memos subfolder.
5. Kennel performs the transfer, stores a versioned copy, and retries failures on a later reconciliation.

The original recording is never deleted or modified. The original M4A container,
including any embedded Apple transcription metadata, is preserved. This version
does not export a separate Markdown transcript or access CloudRecordings.db.

### Setup

- Install Kennel with PR #15 (until that PR is merged).
- Ensure iCloud Voice Memos sync is enabled on both iPhone and Mac.
- Install Google Drive for desktop and confirm a My Drive folder is available
  under ~/Library/CloudStorage/GoogleDrive-<account>/.
- Install and enable this extension. Grant fs_watch, fs_discover and fs_copy.
- In Kennel's Installed tab, select the Google Drive account.

The built-in account picker lists configured Google Drive folders. It does
not need OAuth or the Google Drive API. If Drive is not installed, or an
account uses a different localized folder layout, the picker is empty and
the selected destination must be supported by the Kennel directory provider.

### Limits

- Kernel notifications are not yet wired: current Kennel uses reconciliation
  on scheduled ticks. No manual shell subscription or Python worker is needed.
- macOS Full Disk Access may be required to read Apple's Voice Memos container.
- This does not verify cloud upload completion; Google Drive for desktop owns
  that part of the synchronization.
- Current watcher matches .m4a only. Some Apple Voice Memos recordings use
  other container formats; those are not included yet.
- Apple transcription is preserved if already embedded, but no readable
  Markdown transcript is generated yet.
- Files copied while Drive is offline may remain pending in Drive's queue.

### Dependency

This PR pins the Kennel SDK to the commit of PR #15 to allow its CI to run.
Change the dependency back to main and regenerate Cargo.lock after Kennel
PR #15 is merged.
