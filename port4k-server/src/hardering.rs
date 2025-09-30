pub const MAX_FILES_PER_IMPORT: usize = 500;
pub const MAX_FILE_BYTES: usize = 512 * 1024;            // 512 KB per YAML
pub const MAX_TOTAL_BYTES: usize = 32 * 1024 * 1024;     // 32 MB per import
pub const MAX_LUA_BYTES: usize = 64 * 1024;              // 64 KB per Lua chunk
pub const ALLOW_SYMLINKS: bool = false;

pub const ALLOWED_DIRS: &[&str] = &[
    "north","south","east","west","up","down","in","out",
    "n","s","e","w","u","d"
];

// crude but useful guards (you’re not loading stdlibs, but future-proof anyway)
pub const FORBIDDEN_LUA_TOKENS: &[&str] = &[
    "require", "dofile", "loadfile", "loadstring", "package",
    "io.", "os.", "debug.", "ffi", "collectgarbage", "setfenv", "getfenv"
];