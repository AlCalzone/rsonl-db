#[derive(Debug, Clone, Builder)]
#[builder(default)]
pub struct DBOptions {
  pub(crate) ignore_read_errors: bool,
  // reviver?: (key: string, value: any) => V;
  // serializer?: (key: string, value: V) => any;
  pub(crate) auto_compress: AutoCompressOptions,
  pub(crate) throttle_fs: ThrottleFSOptions,
  pub(crate) lockfile_directory: String,
  pub(crate) index_paths: Vec<String>,
}

impl Default for DBOptions {
  fn default() -> Self {
    Self {
      ignore_read_errors: false,
      auto_compress: AutoCompressOptions::default(),
      throttle_fs: ThrottleFSOptions::default(),
      lockfile_directory: ".".to_owned(),
      index_paths: Vec::new(),
    }
  }
}

#[derive(Debug, Clone, Builder)]
#[builder(default)]
pub struct AutoCompressOptions {
  pub(crate) size_factor: u32,
  pub(crate) size_factor_min_size: u32,
  pub(crate) interval_ms: u32,
  pub(crate) interval_min_changes: u32,
  pub(crate) on_close: bool,
  pub(crate) on_open: bool,
}

impl Default for AutoCompressOptions {
  fn default() -> Self {
    Self {
      size_factor: 0,
      size_factor_min_size: 0,
      interval_ms: 0,
      interval_min_changes: 1,
      on_close: false,
      on_open: false,
    }
  }
}

#[derive(Debug, Clone, Builder)]
#[builder(default)]
pub struct ThrottleFSOptions {
  pub(crate) interval_ms: u32,
  pub(crate) max_buffered_commands: usize,
}

impl Default for ThrottleFSOptions {
  fn default() -> Self {
    Self {
      interval_ms: 0,
      max_buffered_commands: usize::MAX,
    }
  }
}

// export interface JsonlDBOptions<V> {
// 	/**
// 	 * Whether errors reading the db file (e.g. invalid JSON) should silently be ignored.
// 	 * **Warning:** This may result in inconsistent data!
// 	 */
// 	ignoreReadErrors?: boolean;
// 	/**
// 	 * An optional reviver function (similar to JSON.parse) to transform parsed values before they are accessible in the database.
// 	 * If this function is defined, it must always return a value.
// 	 */
// 	reviver?: (key: string, value: any) => V;
// 	/**
// 	 * An optional serializer function (similar to JSON.serialize) to transform values before they are written to the database file.
// 	 * If this function is defined, it must always return a value.
// 	 */
// 	serializer?: (key: string, value: V) => any;

// 	/**
// 	 * Configure when the DB should be automatically compressed.
// 	 * If multiple conditions are configured, the DB is compressed when any of them are fulfilled
// 	 */
// 	autoCompress?: Partial<{
// 		/**
// 		 * Compress when uncompressedSize >= size * sizeFactor. Default: +Infinity
// 		 */
// 		sizeFactor: number;
// 		/**
// 		 * Configure the minimum size necessary for auto-compression based on size. Default: 0
// 		 */
// 		sizeFactorMinimumSize: number;
// 		/**
// 		 * Compress after a certain time has passed. Default: never
// 		 */
// 		intervalMs: number;
// 		/**
// 		 * Configure the minimum count of changes for auto-compression based on time. Default: 1
// 		 */
// 		intervalMinChanges: number;
// 		/** Compress when closing the DB. Default: false */
// 		onClose: boolean;
// 		/** Compress after opening the DB. Default: false */
// 		onOpen: boolean;
// 	}>;

// 	/**
// 	 * Can be used to throttle write accesses to the filesystem. By default,
// 	 * every change is immediately written to the FS
// 	 */
// 	throttleFS?: {
// 		/**
// 		 * Minimum wait time between two consecutive write accesses. Default: 0
// 		 */
// 		intervalMs: number;
// 		/**
// 		 * Maximum commands to be buffered before forcing a write access. Default: +Infinity
// 		 */
// 		maxBufferedCommands?: number;
// 	};

// 	/**
// 	 * Override in which directory the lockfile is created.
// 	 * Defaults to the directory in which the DB file is located.
// 	 */
// 	lockfileDirectory?: string;
// }
