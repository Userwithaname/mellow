//! Error messages for use with `.expect()` and explanations
//! for why the `Err` or `None` variant is not handled.

/// # Reason
/// You guarantee the item was properly initialized and the unwrap will succeed
pub const EXP_INIT: &str = "Item was not properly initialized";
/// # Reason
/// You guarantee that the channel receiver is open and the unwrap will succeed
///
/// Note: For crash handling to work correctly, the main thread should _not_ panic
/// for closed channel errors; channel `send` results should be ignored instead
pub const EXP_RX: &str = "Channel receiver is unavailable";

/// # Reason
/// Panic if initialization fails because the failure case cannot be handled
pub const INIT_ERR: &str = "Initialization failed";
/// # Reason
/// Panic when activating a non-existent `gio` action
pub const ACTION_ERR: &str = "Could not activate action";
