use pickledb::{PickleDb, PickleDbDumpPolicy, SerializationMethod};

pub fn load_db<P: AsRef<std::path::Path>>(output_dir: P) -> PickleDb {
    PickleDb::load(
        output_dir.as_ref().join("photohash.db"),
        PickleDbDumpPolicy::AutoDump,
        SerializationMethod::Json,
    )
    .unwrap_or_else(|_| {
        PickleDb::new(
            output_dir.as_ref().join("photohash.db"),
            PickleDbDumpPolicy::AutoDump,
            SerializationMethod::Json,
        )
    })
}
