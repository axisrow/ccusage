use std::{collections::HashSet, sync::Arc};

use jiff::tz::TimeZone as JiffTimeZone;

use crate::{
    LoadedEntry, PricingMap, TimestampMs, TokenUsageRaw, UsageEntry, UsageMessage,
    calculate_cost_for_usage, cli::CostMode, format_date_tz, format_rfc3339_millis,
    missing_pricing_model_for_candidates,
};

pub(super) struct ZCodeEntry {
    pub(super) id: String,
    pub(super) session_id: String,
    pub(super) model: String,
    pub(super) timestamp: TimestampMs,
    pub(super) directory: Option<String>,
    pub(super) usage: TokenUsageRaw,
    pub(super) reasoning_tokens: u64,
}

pub(super) fn read_model_usage_row(statement: &sqlite::Statement<'_>) -> Option<ZCodeEntry> {
    let id = statement.read::<String, _>(0).ok()?;
    let session_id = statement.read::<String, _>(1).ok()?;
    let model = statement.read::<String, _>(2).ok()?.trim().to_string();
    let started_at = read_i64(statement, 3)?;
    let timestamp = (started_at > 0).then(|| TimestampMs::from_millis(started_at))?;
    if id.is_empty() || session_id.is_empty() || model.is_empty() {
        return None;
    }
    let input_tokens = read_u64(statement, 4);
    let output_tokens = read_u64(statement, 5);
    let reasoning_tokens = read_u64(statement, 6);
    let cache_creation_input_tokens = read_u64(statement, 7);
    let cache_read_input_tokens = read_u64(statement, 8);
    let fresh_input_tokens = input_tokens
        .saturating_sub(cache_read_input_tokens)
        .saturating_sub(cache_creation_input_tokens);
    if fresh_input_tokens == 0
        && output_tokens == 0
        && reasoning_tokens == 0
        && cache_creation_input_tokens == 0
        && cache_read_input_tokens == 0
    {
        return None;
    }
    let directory = statement
        .read::<String, _>(9)
        .ok()
        .filter(|value| !value.trim().is_empty());
    Some(ZCodeEntry {
        id,
        session_id,
        model,
        timestamp,
        directory,
        usage: TokenUsageRaw {
            input_tokens: fresh_input_tokens,
            output_tokens,
            cache_creation_input_tokens,
            cache_read_input_tokens,
            speed: None,
            cache_creation: None,
        },
        reasoning_tokens,
    })
}

fn read_i64(statement: &sqlite::Statement<'_>, index: usize) -> Option<i64> {
    statement.read::<i64, _>(index).ok().or_else(|| {
        statement
            .read::<f64, _>(index)
            .ok()
            .filter(|value| value.is_finite())
            .map(|value| value.trunc() as i64)
    })
}

fn read_u64(statement: &sqlite::Statement<'_>, index: usize) -> u64 {
    read_i64(statement, index)
        .and_then(|value| u64::try_from(value.max(0)).ok())
        .unwrap_or(0)
}

pub(super) fn to_loaded_entry(
    entry: ZCodeEntry,
    tz: Option<&JiffTimeZone>,
    mode: CostMode,
    pricing: &PricingMap,
) -> LoadedEntry {
    let cost_usage = TokenUsageRaw {
        output_tokens: entry
            .usage
            .output_tokens
            .saturating_add(entry.reasoning_tokens),
        cache_creation: None,
        ..entry.usage
    };
    let candidates = model_candidates(&entry.model);
    let cost = candidates
        .iter()
        .map(|candidate| {
            calculate_cost_for_usage(
                Some(candidate),
                cost_usage,
                None,
                mode,
                Some(pricing),
            )
        })
        .find(|cost| *cost > 0.0)
        .unwrap_or(0.0);
    let missing_pricing_model = (mode != CostMode::Display)
        .then(|| {
            missing_pricing_model_for_candidates(
                &entry.model,
                candidates,
                crate::total_usage_tokens(cost_usage),
                Some(pricing),
            )
        })
        .flatten();
    let timestamp_text = format_rfc3339_millis(entry.timestamp);
    let project_path = entry.directory.unwrap_or_else(|| "ZCode".to_string());
    let data = UsageEntry {
        session_id: Some(entry.session_id.clone()),
        timestamp: timestamp_text,
        version: None,
        message: UsageMessage {
            usage: entry.usage,
            model: Some(entry.model.clone()),
            id: Some(format!("zcode:{}", entry.id)),
        },
        cost_usd: None,
        request_id: Some(entry.id),
        is_api_error_message: None,
        is_sidechain: None,
    };
    LoadedEntry {
        date: format_date_tz(entry.timestamp, tz),
        timestamp: entry.timestamp,
        project: Arc::from("zcode"),
        session_id: Arc::from(entry.session_id),
        project_path: Arc::from(project_path),
        cost,
        credits: None,
        extra_total_tokens: entry.reasoning_tokens,
        message_count: None,
        model: Some(entry.model),
        usage_limit_reset_time: None,
        missing_pricing_model,
        data,
    }
}

fn model_candidates(model: &str) -> Vec<String> {
    let candidates = [model.to_string(), format!("zai/{}", model.to_ascii_lowercase())];
    let mut seen = HashSet::new();
    candidates
        .into_iter()
        .filter(|candidate| seen.insert(candidate.clone()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn calculates_glm_5_2_cost_with_fresh_input_and_cached_tokens() {
        let pricing = PricingMap::load_embedded();
        let entry = ZCodeEntry {
            id: "usage-1".to_string(),
            session_id: "session-1".to_string(),
            model: "GLM-5.2".to_string(),
            timestamp: TimestampMs::from_millis(1_735_689_600_123),
            directory: Some("/workspace/zcode".to_string()),
            usage: TokenUsageRaw {
                input_tokens: 700,
                output_tokens: 300,
                cache_creation_input_tokens: 100,
                cache_read_input_tokens: 200,
                speed: None,
                cache_creation: None,
            },
            reasoning_tokens: 50,
        };

        let loaded = to_loaded_entry(entry, None, CostMode::Calculate, &pricing);

        assert_eq!(loaded.data.message.usage.input_tokens, 700);
        assert_eq!(loaded.extra_total_tokens, 50);
        assert!((loaded.cost - 0.002_598).abs() < 1e-12);
    }

    #[test]
    fn display_mode_reports_zero_when_zcode_has_no_recorded_cost() {
        let pricing = PricingMap::load_embedded();
        let entry = ZCodeEntry {
            id: "usage-2".to_string(),
            session_id: "session-2".to_string(),
            model: "GLM-5.2".to_string(),
            timestamp: TimestampMs::from_millis(1_735_689_600_123),
            directory: None,
            usage: TokenUsageRaw {
                input_tokens: 1,
                output_tokens: 1,
                cache_creation_input_tokens: 0,
                cache_read_input_tokens: 0,
                speed: None,
                cache_creation: None,
            },
            reasoning_tokens: 0,
        };

        let loaded = to_loaded_entry(entry, None, CostMode::Display, &pricing);

        assert_eq!(loaded.cost, 0.0);
    }
}
