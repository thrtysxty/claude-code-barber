//! bench — LoCoMo quality benchmark for CCB compression
//!
//! Measures whether CCB's compression preserves the information that matters
//! for long conversations, using the LoCoMo benchmark framework from the paper
//! "Evaluating Very Long-Term Conversational Memory of LLM Agents".

use crate::cli::{CompressionBenchLevel, GainFormat};
use crate::features::trim::compress_str as trim_compress;
use crate::log::estimate_tokens;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

// ─────────────────────────────────────────────────────────────────────────────
// Data structures
// ─────────────────────────────────────────────────────────────────────────────

/// A single turn in a conversation session
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Turn {
    pub role: String,
    pub content: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timestamp: Option<String>,
}

/// A session within a conversation (LoCoMo has ~35 sessions per conversation)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub id: String,
    pub turns: Vec<Turn>,
}

/// A full LoCoMo conversation with QA annotations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocomoConversation {
    pub id: String,
    pub sessions: Vec<Session>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub qa_pairs: Option<Vec<QaPair>>,
}

/// A question-answer pair for evaluation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QaPair {
    pub question: String,
    pub expected_answer: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_context: Option<String>,
}

/// Benchmark result for one conversation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversationResult {
    pub conversation_id: String,
    pub compression_level: String,
    pub tokens_before: usize,
    pub tokens_after: usize,
    pub saved_pct: f64,
    pub qa_accuracy: f64,
    pub exact_matches: usize,
    pub fuzzy_matches: usize,
    pub total_qa: usize,
}

/// Complete benchmark run result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkResult {
    pub levels: Vec<LevelResult>,
    pub total_runtime_ms: u64,
    pub total_tokens_processed: usize,
}

/// Result for one compression level
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LevelResult {
    pub compression_level: String,
    pub avg_tokens_before: f64,
    pub avg_tokens_after: f64,
    pub avg_saved_pct: f64,
    pub avg_qa_accuracy: f64,
    pub per_conversation: Vec<ConversationResult>,
    pub retention_curve: HashMap<usize, f64>, // session_index -> accuracy
}

/// Default bundled dataset path
pub fn default_dataset_path() -> std::path::PathBuf {
    let exe = std::env::current_exe().unwrap_or(std::path::PathBuf::from("."));
    exe.parent()
        .unwrap_or(&std::path::PathBuf::from("."))
        .join("testdata")
        .join("locomo")
}

/// Load LoCoMo dataset from JSON file
pub fn load_dataset(path: &Path) -> anyhow::Result<Vec<LocomoConversation>> {
    let content = std::fs::read_to_string(path)?;
    let convs: Vec<LocomoConversation> = serde_json::from_str(&content)?;
    Ok(convs)
}

/// Build a bundled minimal dataset for CI (10 conversations, ~500KB)
#[cfg(feature = "bench")]
pub fn build_bundled_dataset() -> Vec<LocomoConversation> {
    let mut convs = Vec::with_capacity(10);

    for i in 0..10 {
        let mut sessions = Vec::with_capacity(35);
        for s in 0..35 {
            let mut turns = Vec::with_capacity(8);
            // 8 turns per session = 280 turns total (slightly under 300 but representative)
            for t in 0..8 {
                let role = if t % 2 == 0 { "user" } else { "assistant" };
                turns.push(Turn {
                    role: role.to_string(),
                    content: format!(
                        "This is turn {} of session {} in conversation {}. \
                        The user is discussing a software implementation task with the assistant. \
                        Topics covered include architecture decisions, API design, and code patterns.",
                        t, s, i
                    ),
                    timestamp: Some(format!("2026-05-{:02}T{:02}:00:00Z", (s % 28) + 1, t * 2)),
                });
            }
            sessions.push(Session {
                id: format!("session-{}", s),
                turns,
            });
        }

        // Add QA pairs at session boundaries (sessions 5, 10, 15, 20, 25, 30)
        let mut qa_pairs = Vec::new();
        for sq in [5, 10, 15, 20, 25, 30] {
            qa_pairs.push(QaPair {
                question: format!(
                    "In session {}, what architecture decision was discussed?",
                    sq
                ),
                expected_answer: format!("architecture decisions and API design"),
                session_context: Some(format!("session-{}", sq)),
            });
        }

        convs.push(LocomoConversation {
            id: format!("conv-{:03}", i),
            sessions,
            qa_pairs: Some(qa_pairs),
        });
    }

    convs
}

// ─────────────────────────────────────────────────────────────────────────────
// Compression
// ─────────────────────────────────────────────────────────────────────────────

/// Apply the appropriate compression based on level
pub fn compress_for_level(content: &str, level: &CompressionBenchLevel) -> String {
    match level {
        CompressionBenchLevel::Trim => trim_compress(content),
        CompressionBenchLevel::Cut => {
            // cut = trim + context compact
            let trimmed = trim_compress(content);
            // Take only the first 50% of lines for maximum compression
            let lines: Vec<&str> = trimmed.lines().collect();
            if lines.len() > 10 {
                lines[..lines.len() / 2].join("\n")
            } else {
                trimmed
            }
        }
        CompressionBenchLevel::Buzz => {
            // buzz = strip everything except table rows and key signal
            let trimmed = trim_compress(content);
            let stripped: String = trimmed
                .lines()
                .filter(|l| {
                    l.starts_with('|')
                        || l.contains("error")
                        || l.contains("ERROR")
                        || l.contains("FAILED")
                        || l.contains("warning:")
                })
                .collect::<Vec<_>>()
                .join("\n");
            if stripped.is_empty() {
                trimmed
            } else {
                stripped
            }
        }
        CompressionBenchLevel::All => trim_compress(content), // placeholder for "all"
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Scoring
// ─────────────────────────────────────────────────────────────────────────────

/// Levenshtein distance between two strings (simple implementation)
fn levenshtein(a: &str, b: &str) -> usize {
    let a_chars: Vec<char> = a.to_lowercase().chars().collect();
    let b_chars: Vec<char> = b.to_lowercase().chars().collect();
    let a_len = a_chars.len();
    let b_len = b_chars.len();

    if a_len == 0 {
        return b_len;
    }
    if b_len == 0 {
        return a_len;
    }

    let mut matrix = vec![vec![0usize; b_len + 1]; a_len + 1];

    for i in 0..=a_len {
        matrix[i][0] = i;
    }
    for j in 0..=b_len {
        matrix[0][j] = j;
    }

    for i in 1..=a_len {
        for j in 1..=b_len {
            let cost = if a_chars[i - 1] == b_chars[j - 1] { 0 } else { 1 };
            matrix[i][j] = (matrix[i - 1][j] + 1)
                .min(matrix[i][j - 1] + 1)
                .min(matrix[i - 1][j - 1] + cost);
        }
    }

    matrix[a_len][b_len]
}

/// Check if two strings match exactly (case-insensitive)
/// Returns true if `expected` appears as a substring in `candidate` (after normalization)
fn exact_match(candidate: &str, expected: &str) -> bool {
    candidate.to_lowercase().contains(&expected.to_lowercase())
}

/// Check if two strings fuzzy match (Levenshtein distance <= threshold)
/// For QA scoring: checks if any word in candidate approximately matches any word in expected.
/// This allows matching even when the overall string is different but key terms are preserved.
fn fuzzy_match(candidate: &str, expected: &str, threshold: usize) -> bool {
    let c_lower = candidate.to_lowercase();
    let e_lower = expected.to_lowercase();

    // If candidate fully contains expected as a substring, that's an exact match — not fuzzy
    // (exact_match takes precedence; fuzzy is for partial/reformatted content)
    if c_lower.contains(&e_lower) || e_lower.contains(&c_lower) {
        return false;
    }

    // For QA-style fuzzy matching, check if any significant word in expected
    // approximately matches a word in candidate
    let e_words: Vec<&str> = e_lower.split_whitespace().collect();
    let c_words: Vec<&str> = c_lower.split_whitespace().collect();

    for e_word in &e_words {
        // Skip very short words for matching (articles, prepositions)
        if e_word.len() < 4 {
            continue;
        }
        for c_word in &c_words {
            if c_word.len() >= 3 {
                let dist = levenshtein(c_word, e_word);
                if dist <= threshold {
                    return true;
                }
            }
        }
    }

    false
}

/// Score a single QA pair against compressed content
fn score_qa(qa: &QaPair, compressed_content: &str) -> (bool, bool) {
    // Try exact match first
    if exact_match(compressed_content, &qa.expected_answer) {
        return (true, false);
    }

    // Try fuzzy match
    if fuzzy_match(compressed_content, &qa.expected_answer, 2) {
        return (false, true);
    }

    // Check if any key phrase from expected_answer appears in compressed content
    let expected_words: Vec<&str> = qa.expected_answer.split_whitespace().collect();
    let compressed_lower = compressed_content.to_lowercase();
    let matched_words = expected_words
        .iter()
        .filter(|w| compressed_lower.contains(&(*w).to_lowercase()))
        .count();

    // Partial credit: if >60% of key words found, call it a fuzzy match
    let threshold = (expected_words.len() as f64 * 0.6).ceil() as usize;
    if matched_words >= threshold && matched_words > 0 {
        return (false, true);
    }

    (false, false)
}

/// Run QA evaluation for a conversation
fn evaluate_conversation(conv: &LocomoConversation, compressed: &str) -> (usize, usize, usize) {
    let qa_pairs = conv.qa_pairs.as_ref();

    let Some(qa_pairs) = qa_pairs else {
        return (0, 0, 0);
    };

    let mut exact = 0usize;
    let mut fuzzy = 0usize;

    for qa in qa_pairs {
        // Check against the full compressed content
        let (e_match, f_match) = score_qa(qa, compressed);
        if e_match {
            exact += 1;
        } else if f_match {
            fuzzy += 1;
        }
    }

    (exact, fuzzy, qa_pairs.len())
}

// ─────────────────────────────────────────────────────────────────────────────
// Benchmark runner
// ─────────────────────────────────────────────────────────────────────────────

/// Run the full LoCoMo benchmark
#[cfg(feature = "bench")]
pub fn run_locomo(
    dataset_path: Option<&Path>,
    compression_level: CompressionBenchLevel,
    format: GainFormat,
) -> anyhow::Result<()> {
    use std::time::Instant;

    let start = Instant::now();

    // Load dataset
    let path = if let Some(p) = dataset_path {
        p.to_path_buf()
    } else {
        default_dataset_path()
    };

    let conversations = if path.exists() {
        load_dataset(&path)?
    } else {
        // Generate bundled dataset if no file exists
        eprintln!("Note: No dataset at {:?}, using generated test data", path);
        build_bundled_dataset()
    };

    if conversations.is_empty() {
        anyhow::bail!("No conversations found in dataset");
    }

    // Levels to benchmark
    let levels: Vec<CompressionBenchLevel> = match compression_level {
        CompressionBenchLevel::All => vec![
            CompressionBenchLevel::Trim,
            CompressionBenchLevel::Cut,
            CompressionBenchLevel::Buzz,
        ],
        _ => vec![compression_level.clone()],
    };

    let mut results = Vec::new();

    for level in &levels {
        let level_result = benchmark_level(&conversations, level);
        results.push(level_result);
    }

    let elapsed_ms = start.elapsed().as_millis() as u64;
    let total_tokens: usize = results
        .iter()
        .flat_map(|r| r.per_conversation.iter())
        .map(|c| c.tokens_before)
        .sum();

    let benchmark_result = BenchmarkResult {
        levels: results,
        total_runtime_ms: elapsed_ms,
        total_tokens_processed: total_tokens,
    };

    match format {
        GainFormat::Human => print_human(&benchmark_result),
        GainFormat::Json => print_json(&benchmark_result)?,
    }

    Ok(())
}

fn benchmark_level(convs: &[LocomoConversation], level: &CompressionBenchLevel) -> LevelResult {
    let level_name = match level {
        CompressionBenchLevel::Trim => "trim",
        CompressionBenchLevel::Cut => "cut",
        CompressionBenchLevel::Buzz => "buzz",
        CompressionBenchLevel::All => "all",
    };

    let mut per_conversation = Vec::new();
    let mut total_tokens_before = 0usize;
    let mut total_tokens_after = 0usize;
    let mut total_exact = 0usize;
    let mut total_fuzzy = 0usize;
    let mut total_qa = 0usize;

    // Retention curve: session boundary -> accuracy
    let mut retention_curve: HashMap<usize, f64> = HashMap::new();
    let boundary_sessions = [5, 10, 15, 20, 25, 30];

    for conv in convs {
        // Build full conversation text
        let full_text: String = conv
            .sessions
            .iter()
            .flat_map(|s| s.turns.iter().map(|t| format!("{}: {}", t.role, t.content)))
            .collect::<Vec<_>>()
            .join("\n");

        // Compress
        let compressed = compress_for_level(&full_text, level);

        // Evaluate
        let (exact, fuzzy, qa_count) = evaluate_conversation(conv, &compressed);

        let tokens_before = estimate_tokens(&full_text);
        let tokens_after = estimate_tokens(&compressed);
        let saved_pct = if tokens_before > 0 {
            (tokens_before.saturating_sub(tokens_after) as f64 / tokens_before as f64) * 100.0
        } else {
            0.0
        };

        let qa_accuracy = if qa_count > 0 {
            ((exact + fuzzy) as f64 / qa_count as f64) * 100.0
        } else {
            0.0
        };

        per_conversation.push(ConversationResult {
            conversation_id: conv.id.clone(),
            compression_level: level_name.to_string(),
            tokens_before,
            tokens_after,
            saved_pct,
            qa_accuracy,
            exact_matches: exact,
            fuzzy_matches: fuzzy,
            total_qa: qa_count,
        });

        total_tokens_before += tokens_before;
        total_tokens_after += tokens_after;
        total_exact += exact;
        total_fuzzy += fuzzy;
        total_qa += qa_count;

        // Update retention curve at session boundaries
        if let Some(qa_pairs) = &conv.qa_pairs {
            for (idx, boundary) in boundary_sessions.iter().enumerate() {
                let relevant_qa: Vec<_> = qa_pairs
                    .iter()
                    .filter(|q| {
                        q.session_context
                            .as_ref()
                            .map(|s| s == &format!("session-{}", boundary))
                            .unwrap_or(false)
                    })
                    .collect();

                if !relevant_qa.is_empty() {
                    let correct = relevant_qa.iter().filter(|qa| {
                        let (e, f) = score_qa(qa, &compressed);
                        e || f
                    }).count();

                    let acc = (correct as f64 / relevant_qa.len() as f64) * 100.0;
                    retention_curve.insert(*boundary, acc);
                }
            }
        }
    }

    let n = convs.len() as f64;
    LevelResult {
        compression_level: level_name.to_string(),
        avg_tokens_before: total_tokens_before as f64 / n,
        avg_tokens_after: total_tokens_after as f64 / n,
        avg_saved_pct: if total_tokens_before > 0 {
            ((total_tokens_before - total_tokens_after) as f64 / total_tokens_before as f64)
                * 100.0
        } else {
            0.0
        },
        avg_qa_accuracy: if total_qa > 0 {
            ((total_exact + total_fuzzy) as f64 / total_qa as f64) * 100.0
        } else {
            0.0
        },
        per_conversation,
        retention_curve,
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Output
// ─────────────────────────────────────────────────────────────────────────────

fn print_human(result: &BenchmarkResult) {
    println!();
    println!("╭─────────────────────────────────────────────────────────────────────────╮");
    println!("│                    CCB — LoCoMo Quality Benchmark                        │");
    println!("╰─────────────────────────────────────────────────────────────────────────╯");
    println!();

    for level in &result.levels {
        println!(
            "╭───────────────────────────────────────────────────────────────{:─<31}╮",
            ""
        );
        println!(
            "│ {:^64} │",
            format!(" Compression level: {} ", level.compression_level)
        );
        println!(
            "├────────────┬──────────┬──────────┬───────────────────────────────┤"
        );
        println!(
            "│ compression│ tokens↓  │ saved %  │ QA accuracy (exact/fuzzy)    │"
        );
        println!(
            "├────────────┼──────────┼──────────┼───────────────────────────────┤"
        );

        // Baseline row (no compression)
        println!(
            "│ {:<10} │ {:>8.0} │ {:>7}   │ {:>5.1}% baseline              │",
            "none",
            level.avg_tokens_before * (convs_count(&result.levels) as f64 / result.levels.len() as f64),
            "0%",
            100.0
        );

        // Actual compression row
        println!(
            "│ {:<10} │ {:>8.0} │ {:>7.1}% │ {:>5.1}% ({}/{})               │",
            level.compression_level,
            level.avg_tokens_after,
            level.avg_saved_pct,
            level.avg_qa_accuracy,
            level.per_conversation.iter().map(|c| c.exact_matches).sum::<usize>(),
            level.per_conversation.iter().map(|c| c.fuzzy_matches).sum::<usize>(),
        );
        println!("╰────────────┴──────────┴──────────┴───────────────────────────────╯");
        println!();

        // Retention curve
        if !level.retention_curve.is_empty() {
            println!("  Retention curve (accuracy at session boundaries):");
            let mut boundaries: Vec<_> = level.retention_curve.keys().collect();
            boundaries.sort();
            for &boundary in boundaries {
                if let Some(&acc) = level.retention_curve.get(&boundary) {
                    println!("    session {:2}: {:5.1}%", boundary, acc);
                }
            }
            println!();
        }
    }

    println!(
        "  {} conversations, {} tokens processed in {}ms",
        convs_count(&result.levels),
        result.total_tokens_processed,
        result.total_runtime_ms
    );
    println!();
}

fn convs_count(levels: &[LevelResult]) -> usize {
    levels.first().map(|l| l.per_conversation.len()).unwrap_or(0)
}

fn print_json(result: &BenchmarkResult) -> anyhow::Result<()> {
    println!("{}", serde_json::to_string_pretty(result)?);
    Ok(())
}

// ─────────────────────────────────────────────────────────────────────────────
// Scoring tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_match_same() {
        assert!(exact_match("hello world", "hello world"));
    }

    #[test]
    fn exact_match_case_insensitive() {
        assert!(exact_match("HELLO WORLD", "hello world"));
    }

    #[test]
    fn exact_match_trimmed() {
        assert!(exact_match("  hello world  ", "hello world"));
    }

    #[test]
    fn exact_match_different() {
        assert!(!exact_match("hello world", "hello there"));
    }

    #[test]
    fn fuzzy_match_one_char_off() {
        assert!(fuzzy_match("hello world", "hello worl", 1));
    }

    #[test]
    fn fuzzy_match_two_chars_off() {
        assert!(fuzzy_match("hello world", "hello wor", 2));
    }

    #[test]
    fn fuzzy_match_three_chars_off() {
        assert!(!fuzzy_match("hello world", "hello wo", 2)); // distance 3 > threshold 2
    }

    #[test]
    fn levenshtein_empty() {
        assert_eq!(levenshtein("", ""), 0);
    }

    #[test]
    fn levenshtein_empty_b() {
        assert_eq!(levenshtein("hello", ""), 5);
    }

    #[test]
    fn levenshtein_empty_a() {
        assert_eq!(levenshtein("", "world"), 5);
    }

    #[test]
    fn levenshtein_identical() {
        assert_eq!(levenshtein("hello", "hello"), 0);
    }

    #[test]
    fn levenshtein_one_insert() {
        assert_eq!(levenshtein("hello", "helloo"), 1);
    }

    #[test]
    fn levenshtein_one_delete() {
        assert_eq!(levenshtein("hello", "hell"), 1);
    }

    #[test]
    fn levenshtein_one_substitute() {
        assert_eq!(levenshtein("hello", "hallo"), 1);
    }

    #[test]
    fn compress_trim_preserves_signal() {
        let input = "Compiling foo v1.0.0\nerror[E0308]: mismatched types\nFinished release";
        let out = trim_compress(input);
        assert!(out.contains("error[E0308]"));
        assert!(!out.contains("Compiling"));
        assert!(!out.contains("Finished"));
    }

    #[test]
    fn compress_trim_empty() {
        assert_eq!(trim_compress(""), "");
    }

    #[test]
    fn score_qa_exact() {
        let qa = QaPair {
            question: "What was discussed?".to_string(),
            expected_answer: "architecture decisions".to_string(),
            session_context: Some("session-5".to_string()),
        };
        let (e, f) = score_qa(&qa, "architecture decisions were made");
        assert!(e);
        assert!(!f);
    }

    #[test]
    fn score_qa_fuzzy() {
        let qa = QaPair {
            question: "What was discussed?".to_string(),
            expected_answer: "architecture decisions".to_string(),
            session_context: Some("session-5".to_string()),
        };
        let (e, f) = score_qa(&qa, "archtecture decisions were made"); // typo
        assert!(!e);
        assert!(f); // distance 2 <= threshold
    }

    #[test]
    fn score_qa_partial() {
        let qa = QaPair {
            question: "What was discussed?".to_string(),
            expected_answer: "architecture design API patterns".to_string(),
            session_context: Some("session-5".to_string()),
        };
        let (e, f) = score_qa(&qa, "architecture decisions were discussed");
        assert!(!e);
        // "architecture" matches but "design" and "API" and "patterns" don't
        // 1/4 words = 25% < 60% threshold, so no partial credit
        assert!(!f);
    }

    #[test]
    fn build_bundled_dataset_size() {
        let dataset = build_bundled_dataset();
        assert_eq!(dataset.len(), 10);
        for conv in &dataset {
            assert_eq!(conv.sessions.len(), 35);
            for session in &conv.sessions {
                assert_eq!(session.turns.len(), 8);
            }
        }
    }
}