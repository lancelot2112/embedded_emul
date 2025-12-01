#[derive(Clone, Copy)]
pub struct AccessContext {
    /// If true, this is a "debugger" view:
    /// 1. Do NOT trigger side effects (e.g., clear-on-read
    /// 2. Do NOT block (if simulating blocking IO).
    /// 3. Bypass Read-Only checks (allow force-writes to RO regions/registers)
    pub debug: bool,
    // This can be extended with other hardware attributes
    // Privelege Level (User/Supervisor
    // Trust zone
}

impl AccessContext {
    pub const CPU: Self = Self { debug: false };
    pub const DEBUG: Self = Self { debug: true };
}
