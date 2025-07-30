//! Progress reporting and warning system for conversions.

use std::collections::HashMap;
use std::time::{Duration, Instant};

/// Severity levels for warnings and messages.
#[derive(Debug, Clone, PartialEq)]
pub enum Severity {
    Info,
    Warning,
    Error,
}

/// A warning or informational message during conversion.
#[derive(Debug, Clone)]
pub struct ConversionMessage {
    pub severity: Severity,
    pub message: String,
    pub context: Option<String>,
    pub element_path: Option<String>,
}

/// Progress tracker for conversion operations.
#[derive(Debug)]
pub struct ProgressTracker {
    /// Start time of the operation
    start_time: Instant,
    /// Total number of items to process
    total_items: usize,
    /// Number of items processed
    processed_items: usize,
    /// Current operation description
    current_operation: String,
    /// Collected warnings and messages
    messages: Vec<ConversionMessage>,
    /// Statistics about the conversion
    stats: ConversionStats,
}

/// Statistics collected during conversion.
#[derive(Debug, Default)]
pub struct ConversionStats {
    pub elements_processed: usize,
    pub constraints_processed: usize,
    pub slices_processed: usize,
    pub references_resolved: usize,
    pub warnings_generated: usize,
    pub errors_encountered: usize,
}

impl ConversionMessage {
    /// Create a new informational message.
    pub fn info(message: String) -> Self {
        Self {
            severity: Severity::Info,
            message,
            context: None,
            element_path: None,
        }
    }

    /// Create a new warning message.
    pub fn warning(message: String) -> Self {
        Self {
            severity: Severity::Warning,
            message,
            context: None,
            element_path: None,
        }
    }

    /// Create a new error message.
    pub fn error(message: String) -> Self {
        Self {
            severity: Severity::Error,
            message,
            context: None,
            element_path: None,
        }
    }

    /// Add context to the message.
    pub fn with_context(mut self, context: String) -> Self {
        self.context = Some(context);
        self
    }

    /// Add element path to the message.
    pub fn with_element_path(mut self, path: String) -> Self {
        self.element_path = Some(path);
        self
    }
}

impl ProgressTracker {
    /// Create a new progress tracker.
    pub fn new(total_items: usize, operation: String) -> Self {
        Self {
            start_time: Instant::now(),
            total_items,
            processed_items: 0,
            current_operation: operation,
            messages: Vec::new(),
            stats: ConversionStats::default(),
        }
    }

    /// Update progress with the number of items processed.
    pub fn update_progress(&mut self, processed: usize) {
        self.processed_items = processed;
    }

    /// Increment processed items by one.
    pub fn increment_progress(&mut self) {
        self.processed_items += 1;
    }

    /// Set the current operation description.
    pub fn set_operation(&mut self, operation: String) {
        self.current_operation = operation;
    }

    /// Add a message to the tracker.
    pub fn add_message(&mut self, message: ConversionMessage) {
        match message.severity {
            Severity::Warning => self.stats.warnings_generated += 1,
            Severity::Error => self.stats.errors_encountered += 1,
            _ => {}
        }
        self.messages.push(message);
    }

    /// Add an info message.
    pub fn info(&mut self, message: String) {
        self.add_message(ConversionMessage::info(message));
    }

    /// Add a warning message.
    pub fn warn(&mut self, message: String) {
        self.add_message(ConversionMessage::warning(message));
    }

    /// Add an error message.
    pub fn error(&mut self, message: String) {
        self.add_message(ConversionMessage::error(message));
    }

    /// Add a warning with context.
    pub fn warn_with_context(&mut self, message: String, context: String) {
        self.add_message(ConversionMessage::warning(message).with_context(context));
    }

    /// Add a warning with element path.
    pub fn warn_with_path(&mut self, message: String, path: String) {
        self.add_message(ConversionMessage::warning(message).with_element_path(path));
    }

    /// Update statistics.
    pub fn update_stats(&mut self, stat_type: StatType, count: usize) {
        match stat_type {
            StatType::ElementsProcessed => self.stats.elements_processed += count,
            StatType::ConstraintsProcessed => self.stats.constraints_processed += count,
            StatType::SlicesProcessed => self.stats.slices_processed += count,
            StatType::ReferencesResolved => self.stats.references_resolved += count,
        }
    }

    /// Get current progress as a percentage.
    pub fn progress_percentage(&self) -> f64 {
        if self.total_items == 0 {
            return 100.0;
        }
        (self.processed_items as f64 / self.total_items as f64) * 100.0
    }

    /// Get elapsed time since start.
    pub fn elapsed_time(&self) -> Duration {
        self.start_time.elapsed()
    }

    /// Get estimated time remaining.
    pub fn estimated_time_remaining(&self) -> Option<Duration> {
        if self.processed_items == 0 {
            return None;
        }

        let elapsed = self.elapsed_time();
        let rate = self.processed_items as f64 / elapsed.as_secs_f64();
        let remaining_items = self.total_items.saturating_sub(self.processed_items);

        if rate > 0.0 {
            Some(Duration::from_secs_f64(remaining_items as f64 / rate))
        } else {
            None
        }
    }

    /// Check if conversion is complete.
    pub fn is_complete(&self) -> bool {
        self.processed_items >= self.total_items
    }

    /// Get all messages.
    pub fn messages(&self) -> &[ConversionMessage] {
        &self.messages
    }

    /// Get messages by severity.
    pub fn messages_by_severity(&self, severity: Severity) -> Vec<&ConversionMessage> {
        self.messages.iter()
            .filter(|msg| msg.severity == severity)
            .collect()
    }

    /// Get conversion statistics.
    pub fn stats(&self) -> &ConversionStats {
        &self.stats
    }

    /// Get current operation.
    pub fn current_operation(&self) -> &str {
        &self.current_operation
    }

    /// Generate a progress report.
    pub fn generate_report(&self) -> ProgressReport {
        ProgressReport {
            progress_percentage: self.progress_percentage(),
            processed_items: self.processed_items,
            total_items: self.total_items,
            current_operation: self.current_operation.clone(),
            elapsed_time: self.elapsed_time(),
            estimated_time_remaining: self.estimated_time_remaining(),
            stats: self.stats.clone(),
            warning_count: self.stats.warnings_generated,
            error_count: self.stats.errors_encountered,
        }
    }
}

/// Types of statistics that can be updated.
pub enum StatType {
    ElementsProcessed,
    ConstraintsProcessed,
    SlicesProcessed,
    ReferencesResolved,
}

/// A progress report snapshot.
#[derive(Debug, Clone)]
pub struct ProgressReport {
    pub progress_percentage: f64,
    pub processed_items: usize,
    pub total_items: usize,
    pub current_operation: String,
    pub elapsed_time: Duration,
    pub estimated_time_remaining: Option<Duration>,
    pub stats: ConversionStats,
    pub warning_count: usize,
    pub error_count: usize,
}

impl ProgressReport {
    /// Format the progress report as a human-readable string.
    pub fn format(&self) -> String {
        let mut report = format!(
            "Progress: {:.1}% ({}/{}) - {}",
            self.progress_percentage,
            self.processed_items,
            self.total_items,
            self.current_operation
        );

        report.push_str(&format!("\nElapsed: {:.2}s", self.elapsed_time.as_secs_f64()));

        if let Some(eta) = self.estimated_time_remaining {
            report.push_str(&format!(" | ETA: {:.2}s", eta.as_secs_f64()));
        }

        if self.warning_count > 0 || self.error_count > 0 {
            report.push_str(&format!(" | Warnings: {} | Errors: {}", self.warning_count, self.error_count));
        }

        report.push_str(&format!(
            "\nStats: Elements: {}, Constraints: {}, Slices: {}, References: {}",
            self.stats.elements_processed,
            self.stats.constraints_processed,
            self.stats.slices_processed,
            self.stats.references_resolved
        ));

        report
    }
}

impl Clone for ConversionStats {
    fn clone(&self) -> Self {
        Self {
            elements_processed: self.elements_processed,
            constraints_processed: self.constraints_processed,
            slices_processed: self.slices_processed,
            references_resolved: self.references_resolved,
            warnings_generated: self.warnings_generated,
            errors_encountered: self.errors_encountered,
        }
    }
}
