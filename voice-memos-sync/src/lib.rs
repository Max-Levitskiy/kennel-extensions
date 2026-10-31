use kennel_guest_sdk::{
    kennel_fs_extension, host_fs_copy, ConfigField, DirectoryOptions,
    FileMarker, FsEvent, FsKind, FsSubscription, Handler, Manifest, Status,
};

#[derive(Default)]
struct VoiceMemos;

struct M4aFile;

impl FileMarker for M4aFile {
    const EXTENSION: &'static str = ".m4a";
}

impl Handler<FsEvent<M4aFile>> for VoiceMemos {
    fn on_event(&self, event: FsEvent<M4aFile>) -> Result<(), String> {
        if !host_fs_copy(event.file(), "Voice Memos") {
            return Err("Kennel refused to queue the recording copy; select a Drive account".into());
        }
        Ok(())
    }

    fn on_error(&self, error: &str) -> Result<(), String> {
        Err(format!("Voice Memos copy failed (will retry): {error}"))
    }
}

fn my_manifest() -> Manifest {
    Manifest {
        name: "voice-memos-sync",
        version: "0.1.0",
        description: "Backs up synced Apple Voice Memos to your selected Google Drive account",
        interval_secs: 60,
        capabilities: vec!["fs_watch", "fs_discover", "fs_copy"],
        fs_subscriptions: vec![FsSubscription {
            id: "recordings",
            path: "~/Library/Group Containers/group.com.apple.VoiceMemos.shared/Recordings",
            pattern: "*.m4a",
            kind: FsKind::File,
            recursive: true,
        }],
        config_fields: vec![ConfigField {
            name: "drive",
            label: "Google Drive account",
            directories: DirectoryOptions {
                root: "~/Library/CloudStorage",
                prefix: "GoogleDrive-",
                child: "My Drive",
            },
        }],
        ..Default::default()
    }
}

fn my_check() -> Status { Status::Healthy }
fn my_fix() {}

kennel_fs_extension!(my_manifest, my_check, my_fix, VoiceMemos, M4aFile);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifest_declares_one_m4a_source_and_one_drive_picker() {
        let m = my_manifest();
        assert_eq!(m.fs_subscriptions.len(), 1);
        assert_eq!(m.fs_subscriptions[0].pattern, "*.m4a");
        assert_eq!(m.config_fields.len(), 1);
        assert_eq!(m.config_fields[0].name, "drive");
    }
}
