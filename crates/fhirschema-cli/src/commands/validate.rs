//! Validate command implementation.

use clap::Args;
use std::path::PathBuf;
use std::fs;
use fhirschema_core::Schema;
use fhirschema_validator::{Validator, Severity};

/// Validate FHIR resource against FHIRSchema
#[derive(Args)]
pub struct ValidateCommand {
    /// FHIRSchema file path
    #[arg(short, long)]
    pub schema: PathBuf,

    /// FHIR resource file path to validate
    #[arg(short, long)]
    pub resource: PathBuf,

    /// Verbose output
    #[arg(short, long)]
    pub verbose: bool,
}

impl ValidateCommand {
    /// Execute the validate command
    pub fn execute(&self) -> anyhow::Result<()> {
        println!("Validating {} against schema {}",
                 self.resource.display(),
                 self.schema.display());

        if self.verbose {
            println!("Verbose mode enabled");
        }

        // Load the FHIRSchema
        let schema_content = fs::read_to_string(&self.schema)?;
        let schema: Schema = if self.schema.extension().and_then(|s| s.to_str()) == Some("yaml") {
            serde_yaml::from_str(&schema_content)?
        } else {
            serde_json::from_str(&schema_content)?
        };

        // Load the FHIR resource
        let resource_content = fs::read_to_string(&self.resource)?;
        let resource: serde_json::Value = if self.resource.extension().and_then(|s| s.to_str()) == Some("yaml") {
            serde_yaml::from_str(&resource_content)?
        } else {
            serde_json::from_str(&resource_content)?
        };

        // Create validator and add schema
        let mut validator = Validator::new();
        let schema_url = schema.url.clone();
        validator.add_schema(schema)?;

        // Validate the resource
        let result = validator.validate(&resource, &schema_url)?;

        // Report results
        if result.success {
            println!("✅ Validation successful!");
        } else {
            println!("❌ Validation failed with {} issues:", result.issues.len());
        }

        // Display issues
        for issue in &result.issues {
            let icon = match issue.severity {
                Severity::Error => "🔴",
                Severity::Warning => "🟡",
                Severity::Information => "🔵",
            };

            println!("{} [{}] {} at {}: {}",
                     icon,
                     format!("{:?}", issue.severity).to_uppercase(),
                     issue.code,
                     issue.location,
                     issue.message);

            if self.verbose {
                if let Some(context) = &issue.context {
                    println!("   Context: {}", context);
                }
            }
        }

        // Display statistics
        if self.verbose {
            println!("\nValidation Statistics:");
            println!("  Elements validated: {}", result.stats.elements_validated);
            println!("  Constraints evaluated: {}", result.stats.constraints_evaluated);
            println!("  Primitives validated: {}", result.stats.primitives_validated);
            println!("  Duration: {}ms", result.stats.duration_ms);
        }

        // Exit with error code if validation failed
        if !result.success {
            std::process::exit(1);
        }

        Ok(())
    }
}
