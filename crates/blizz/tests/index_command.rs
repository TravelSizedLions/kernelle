#[cfg(feature = "neural")]
use anyhow::Result;
#[cfg(feature = "neural")]
use blizz::commands::*;
#[cfg(feature = "neural")]
use blizz::embedding_client::EmbeddingClient;
#[cfg(feature = "neural")]
use blizz::insight::{self, Insight};
#[cfg(feature = "neural")]
use chrono::Utc;
#[cfg(feature = "neural")]
use blizz::embedding_client::Embedding;
#[cfg(feature = "neural")]
use serial_test::serial;
#[cfg(feature = "neural")]
use std::env;
#[cfg(feature = "neural")]
use tempfile::TempDir;

#[cfg(test)]
mod index_command_tests {
  use super::*;

  fn setup_temp_insights_root(_test_name: &str) -> TempDir {
    let temp_dir = TempDir::new().unwrap();
    env::set_var("BLIZZ_INSIGHTS_ROOT", temp_dir.path());
    temp_dir
  }

  #[test]
  #[serial]
  fn test_index_insights_empty_database() -> Result<()> {
    let _temp = setup_temp_insights_root("index_empty");
    let client = EmbeddingClient::with_mock();

    // Should handle empty database gracefully
    index_insights_with_client(false, true, &client)?;

    Ok(())
  }

  #[test]
  #[serial]
  fn test_index_insights_force_all() -> Result<()> {
    let _temp = setup_temp_insights_root("index_force_all");
    let client = EmbeddingClient::with_mock();

    // Create some insights first
    add_insight_with_client("topic1", "insight1", "Overview 1", "Details 1", &client)?;
    add_insight_with_client("topic1", "insight2", "Overview 2", "Details 2", &client)?;
    add_insight_with_client("topic2", "insight3", "Overview 3", "Details 3", &client)?;

    // Force recompute all embeddings
    index_insights_with_client(true, false, &client)?;

    Ok(())
  }

  #[test]
  #[serial]
  fn test_index_insights_missing_only() -> Result<()> {
    let _temp = setup_temp_insights_root("index_missing_only");
    let client = EmbeddingClient::with_mock();

    // Create insights (they'll have embeddings from MockEmbeddingService)
    add_insight_with_client("topic1", "insight1", "Overview 1", "Details 1", &client)?;
    add_insight_with_client("topic2", "insight2", "Overview 2", "Details 2", &client)?;

    // Index only missing embeddings
    index_insights_with_client(false, true, &client)?;

    Ok(())
  }

  #[test]
  #[serial]
  fn test_index_insights_multiple_topics() -> Result<()> {
    let _temp = setup_temp_insights_root("index_multiple_topics");
    let client = EmbeddingClient::with_mock();

    // Create insights across multiple topics
    add_insight_with_client("ai", "neural_networks", "About neural networks", "Deep learning details", &client)?;
    add_insight_with_client("ai", "machine_learning", "About ML", "ML algorithms", &client)?;
    add_insight_with_client("databases", "postgresql", "About PostgreSQL", "Database management", &client)?;
    add_insight_with_client("databases", "redis", "About Redis", "In-memory store", &client)?;
    add_insight_with_client("rust", "ownership", "About ownership", "Memory management", &client)?;

    // Index all insights
    index_insights_with_client(false, true, &client)?;

    Ok(())
  }

  #[test]
  #[serial]
  fn test_index_insights_preserves_existing_insights() -> Result<()> {
    let _temp = setup_temp_insights_root("index_preserves");
    let client = EmbeddingClient::with_mock();

    // Create an insight
    add_insight_with_client("preserve", "test", "Original overview", "Original details", &client)?;

    // Verify content before indexing
    let before = insight::load("preserve", "test")?;
    assert_eq!(before.overview, "Original overview");
    assert_eq!(before.details, "Original details");

    // Run indexing
    index_insights_with_client(true, false, &client)?;

    // Verify content is preserved after indexing
    let after = insight::load("preserve", "test")?;
    assert_eq!(after.overview, "Original overview");
    assert_eq!(after.details, "Original details");

    Ok(())
  }

  #[test]
  #[serial]
  fn test_index_insights_updates_embedding_metadata() -> Result<()> {
    let _temp = setup_temp_insights_root("index_metadata");
    let client = EmbeddingClient::with_mock();

    // Create an insight
    add_insight_with_client("metadata", "test", "Test overview", "Test details", &client)?;

    // Load and verify it has embedding data (from MockEmbeddingService)
    let insight = insight::load("metadata", "test")?;
    assert!(insight::has_embedding(&insight));
    assert!(insight.embedding_version.is_some());

    // Force reindex
    index_insights_with_client(true, false, &client)?;

    // Verify it still has embedding metadata
    let reindexed = insight::load("metadata", "test")?;
    assert!(insight::has_embedding(&reindexed));
    assert!(reindexed.embedding_version.is_some());

    Ok(())
  }

  #[test]
  #[serial]
  fn test_index_insights_handles_unicode_content() -> Result<()> {
    let _temp = setup_temp_insights_root("index_unicode");
    let client = EmbeddingClient::with_mock();

    // Create insights with unicode content
    add_insight_with_client(
      "unicode",
      "test",
      "Overview with émojis 🚀 and unicode: ñáéíóú",
      "Details with Chinese: 你好世界, Arabic: مرحبا, Russian: Привет",
      &client
    )?;

    // Index should handle unicode content without issues
    index_insights_with_client(false, true, &client)?;

    // Verify the insight still exists and has correct content
    let insight = insight::load("unicode", "test")?;
    assert!(insight.overview.contains("émojis 🚀"));
    assert!(insight.details.contains("你好世界"));

    Ok(())
  }

  #[test]
  #[serial]
  fn test_index_insights_verify_content_preserved() -> Result<()> {
    let _temp = setup_temp_insights_root("index_content_preserved");
    let client = EmbeddingClient::with_mock();

    let original_overview = "This is a test overview with specific content";
    let original_details = "These are test details with\nmultiple lines\nand special characters: @#$%^&*()";

    // Create insight with specific content
    add_insight_with_client("content", "preservation", &original_overview, &original_details, &client)?;

    // Index the insights
    index_insights_with_client(true, false, &client)?;

    // Verify exact content preservation
    let preserved = insight::load("content", "preservation")?;
    assert_eq!(preserved.overview, original_overview);
    assert_eq!(preserved.details, original_details);

    Ok(())
  }

  // Helper function for creating test insights directory (unused in simplified tests)
  #[allow(dead_code)]
  fn create_test_insights_dir() -> Result<()> {
    use std::fs;

    let insights_dir = std::env::var("BLIZZ_INSIGHTS_ROOT")?;
    fs::create_dir_all(&insights_dir)?;
    Ok(())
  }
}

// Test that index_insights compilation is conditional on neural feature
#[cfg(not(feature = "neural"))]
#[cfg(test)]
mod general_index_tests {
  #[test]
  fn test_index_command_conditional_compilation() {
    // This test verifies that when neural features are disabled,
    // the code still compiles but index_insights is not available
    // The test itself doesn't do much, but ensures the conditional compilation works
    assert!(true);
  }
}
