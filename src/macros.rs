/// Logs an event to `AppState` and prints it to stdout with `[EdgeBridge]`.
///
/// ```
/// use edgebridge::models::{AppState, NodeRole};
/// let mut st = AppState::new_listening(
///     "n1".into(), "node-a".into(), "192.168.1.10".into(), "192.168.1.0/24".into()
/// );
/// edgebridge::eb_log!(st, "booted {}", st.role.label());
/// assert_eq!(st.logs.len(), 1);
/// assert!(st.logs[0].contains("booted LISTENING"));
/// ```
#[macro_export]
macro_rules! eb_log {
    ($state:expr, $($arg:tt)*) => {{
        let __msg = format!($($arg)*);$state.push_log(__msg);
    }};
}

/// Checks an invariant in both debug and release builds; logs on failure.
///
/// ```
/// use edgebridge::models::AppState;
/// let mut st = AppState::new_listening(
///     "n1".into(), "node-a".into(), "10.0.0.2".into(), "10.0.0.0/24".into()
/// );
/// assert!(edgebridge::eb_invariant!(st, 1 + 1 == 2, "math check"));
/// assert!(!edgebridge::eb_invariant!(st, false, "forced failure"));
/// assert!(st.logs[0].contains("INVARIANT VIOLATION: forced failure"));
/// ```
#[macro_export]
macro_rules! eb_invariant {
    ($state:expr,$cond:expr, $($arg:tt)*) => {{
        if !($cond) {
            let __detail = format!($($arg)*);
            $crate::eb_log!($state, "INVARIANT VIOLATION: {}", __detail);
            false
        } else {
            true
        }
    }};
}
