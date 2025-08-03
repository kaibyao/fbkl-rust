// Module declarations - organize the deadline configuration functionality
pub mod configuration;
pub mod generation;
pub mod validation;

// Re-export public API for easy access
pub use configuration::{
    activate_deadlines, can_edit_config, configure_deadlines, delete_deadline_config,
    get_deadline_config,
};
pub use generation::{GeneratedDeadline, get_deadline_dependency_order};
pub use validation::{DeadlineConfigInput, ValidationError, ValidationResult};
