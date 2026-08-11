use serde_json::{Value, json};

use crate::{
    BucketKind, LoadedEntry, Result,
    adapter::opencode,
    cli::{AgentReportKind, WeekDay},
    summarize_by_key, summarize_summaries_by_bucket, totals_json,
};

pub(crate) fn report_from_rows(rows: &[crate::UsageSummary], kind: AgentReportKind) -> Value {
    let rows_json = rows
        .iter()
        .map(|row| opencode::agent_summary_json(row, kind, false))
        .collect::<Vec<_>>();
    json!({
        rows_key(kind): rows_json,
        "totals": totals_json(rows),
    })
}

pub(crate) fn summarize_entries(
    entries: &[LoadedEntry],
    kind: AgentReportKind,
) -> Result<Vec<crate::UsageSummary>> {
    match kind {
        AgentReportKind::Daily => summarize_by_key(
            entries,
            |entry| entry.date.clone(),
            |date| (date.to_string(), None),
        ),
        AgentReportKind::Monthly => {
            let daily = summarize_entries(entries, AgentReportKind::Daily)?;
            Ok(summarize_summaries_by_bucket(
                &daily,
                BucketKind::Monthly,
                WeekDay::Sunday,
            ))
        }
        AgentReportKind::Session => summarize_by_key(
            entries,
            |entry| entry.session_id.to_string(),
            |session_id| (session_id.to_string(), None),
        )
        .map(|mut rows| {
            for row in &mut rows {
                row.session_id = row.date.take();
            }
            rows
        }),
        AgentReportKind::Weekly => {
            let daily = summarize_entries(entries, AgentReportKind::Daily)?;
            Ok(summarize_summaries_by_bucket(
                &daily,
                BucketKind::Weekly,
                WeekDay::Sunday,
            ))
        }
    }
}

fn rows_key(kind: AgentReportKind) -> &'static str {
    match kind {
        AgentReportKind::Daily => "daily",
        AgentReportKind::Weekly => "weekly",
        AgentReportKind::Monthly => "monthly",
        AgentReportKind::Session => "sessions",
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use crate::{TokenUsageRaw, UsageEntry, UsageMessage};

    #[test]
    fn aggregates_zcode_fresh_input_cache_and_reasoning_tokens() {
        let entry = LoadedEntry {
            data: UsageEntry {
                session_id: Some("session-1".to_string()),
                timestamp: "2025-01-01T00:00:00.123Z".to_string(),
                version: None,
                message: UsageMessage {
                    usage: TokenUsageRaw {
                        input_tokens: 700,
                        output_tokens: 300,
                        cache_creation_input_tokens: 100,
                        cache_read_input_tokens: 200,
                        speed: None,
                        cache_creation: None,
                    },
                    model: Some("GLM-5.2".to_string()),
                    id: Some("zcode:1".to_string()),
                },
                cost_usd: None,
                request_id: Some("1".to_string()),
                is_api_error_message: None,
                is_sidechain: None,
            },
            timestamp: crate::TimestampMs::from_millis(1_735_689_600_123),
            date: "2025-01-01".to_string(),
            project: Arc::from("zcode"),
            session_id: Arc::from("session-1"),
            project_path: Arc::from("/workspace/zcode"),
            cost: 0.002_598,
            credits: None,
            extra_total_tokens: 50,
            message_count: None,
            model: Some("GLM-5.2".to_string()),
            usage_limit_reset_time: None,
            missing_pricing_model: None,
        };

        let rows = summarize_entries(&[entry], AgentReportKind::Daily).unwrap();
        let report = report_from_rows(&rows, AgentReportKind::Daily);

        assert_eq!(report["daily"][0]["inputTokens"], 700);
        assert_eq!(report["daily"][0]["cacheCreationTokens"], 100);
        assert_eq!(report["daily"][0]["cacheReadTokens"], 200);
        assert_eq!(report["daily"][0]["totalTokens"], 1_350);
    }
}
