// Copyright (c) 2019-2026 Provable Inc.
// This file is part of the snarkVM library.

// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at:

// http://www.apache.org/licenses/LICENSE-2.0

// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use crate::{Attribution, MapKey, MapStats, estimate_num_keys, map_label, sst_estimated_active_keys};

/// Paths and network id of the instance that was read.
pub struct ReportHeader {
    pub primary_path: String,
    pub secondary_path: String,
    pub network_id: u16,
}

/// RocksDB properties that go into `rocksdb.estimate-num-keys`, plus a few witnesses.
pub struct Gauge {
    pub estimate_num_keys: Option<u64>,
    pub active_mem_entries: u64,
    pub imm_mem_entries: u64,
    pub active_mem_deletions: u64,
    pub imm_mem_deletions: u64,
    pub num_snapshots: u64,
    pub num_live_versions: u64,
    pub live_sst_files_size: Option<u64>,
}

/// Renders the gauge and the per-map SST attribution.
pub fn render(header: &ReportHeader, gauge: &Gauge, attribution: &Attribution) -> String {
    let totals = attribution.totals();
    let mem_entries = gauge.active_mem_entries.saturating_add(gauge.imm_mem_entries);
    let mem_deletions = gauge.active_mem_deletions.saturating_add(gauge.imm_mem_deletions);
    let sst_active = sst_estimated_active_keys(totals.entries, totals.deletions);
    let recomputed = estimate_num_keys(sst_active, mem_entries, mem_deletions);
    let show_network = network_count(attribution) > 1;

    let mut out = String::new();
    push(&mut out, &format!("primary:   {}", header.primary_path));
    push(&mut out, &format!("secondary: {}", header.secondary_path));
    push(&mut out, &format!("network:   {}", header.network_id));
    push(&mut out, "");
    push(
        &mut out,
        "rocksdb.estimate-num-keys is one integer for the default column family. snarkVM stores every ledger map in that column family; the map is the second little-endian u16 in the key. RocksDB has no property that splits the gauge by key prefix.",
    );
    push(
        &mut out,
        "Each SST file below is charged to the map that contains its smallest and largest key. A file whose endpoints sit in different maps is split in proportion to the live keys in that range. The entries and deletions are the SST inputs of the gauge: estimated active keys = entries - 2 * deletions, then clamped to zero for the whole database.",
    );
    push(&mut out, "");
    push(&mut out, &format!("rocksdb.estimate-num-keys: {}", format_option(gauge.estimate_num_keys)));
    push(&mut out, &format!("recomputed from live SST files and memtables: {}", grouped(recomputed)));
    match gauge.estimate_num_keys {
        Some(property) if property == recomputed => {
            push(&mut out, "recomputed value matches the property.");
        }
        Some(property) => {
            let delta = i128::from(property) - i128::from(recomputed);
            push(
                &mut out,
                &format!(
                    "recomputed value differs from the property by {}. RocksDB scales the SST term when some files have no table statistics, and live file metadata can include SSTs outside the current LSM version.",
                    grouped_i128(delta)
                ),
            );
        }
        None => push(&mut out, "rocksdb.estimate-num-keys was not reported."),
    }
    push(&mut out, &format!("  sst files: {}", grouped(totals.files)));
    push(&mut out, &format!("  sst files split across maps: {}", grouped(attribution.spanning_files)));
    push(&mut out, &format!("  sst entries: {}", grouped(totals.entries)));
    push(&mut out, &format!("  sst deletions: {}", grouped(totals.deletions)));
    push(&mut out, &format!("  sst estimated active keys: {}", grouped(sst_active)));
    push(
        &mut out,
        &format!(
            "  memtable entries: active {}, immutable {}",
            grouped(gauge.active_mem_entries),
            grouped(gauge.imm_mem_entries)
        ),
    );
    push(
        &mut out,
        &format!(
            "  memtable deletions: active {}, immutable {}",
            grouped(gauge.active_mem_deletions),
            grouped(gauge.imm_mem_deletions)
        ),
    );
    push(&mut out, "  memtable keys are part of the gauge and are not included in the per-map table.");
    push(&mut out, &format!("  snapshots: {}", grouped(gauge.num_snapshots)));
    push(&mut out, &format!("  live versions: {}", grouped(gauge.num_live_versions)));
    push(&mut out, &format!("  live SST bytes in file metadata: {}", grouped(totals.bytes)));
    push(&mut out, &format!("  rocksdb.live-sst-files-size: {}", format_option(gauge.live_sst_files_size)));
    if attribution.spanning_files > 0 {
        push(&mut out, &format!("  live keys read from spanning files: {}", grouped(attribution.scanned_live_keys)));
    }
    push(&mut out, "");
    push(&mut out, "Category rollup");
    push_table(&mut out, &category_rows(attribution, show_network, sst_active));
    push(&mut out, "");
    push(&mut out, "Per-map SST contribution");
    push_table(&mut out, &map_rows(attribution, show_network, sst_active));
    push(&mut out, "");
    push(&mut out, "A stock RocksDB metric cannot replace this table. See ledger/rocksdb-analyser/README.md.");
    out
}

fn network_count(attribution: &Attribution) -> usize {
    let mut networks = Vec::new();
    for key in attribution.maps.keys() {
        if let MapKey::Map { network_id, .. } = key
            && !networks.contains(network_id)
        {
            networks.push(*network_id);
        }
    }
    networks.len()
}

fn category_rows(attribution: &Attribution, show_network: bool, sst_active: u64) -> Vec<Vec<String>> {
    let mut grouped_stats: Vec<(String, MapStats)> = Vec::new();
    for (key, stats) in &attribution.maps {
        let label = category_label(*key, show_network);
        if let Some((_, existing)) = grouped_stats.iter_mut().find(|(name, _)| *name == label) {
            existing.files += stats.files;
            existing.split_files += stats.split_files;
            existing.entries += stats.entries;
            existing.deletions += stats.deletions;
            existing.bytes += stats.bytes;
        } else {
            grouped_stats.push((label, *stats));
        }
    }
    rows(grouped_stats, sst_active)
}

fn map_rows(attribution: &Attribution, show_network: bool, sst_active: u64) -> Vec<Vec<String>> {
    let labeled = attribution.maps.iter().map(|(key, stats)| (row_label(*key, show_network), *stats)).collect();
    rows(labeled, sst_active)
}

fn rows(mut labeled: Vec<(String, MapStats)>, sst_active: u64) -> Vec<Vec<String>> {
    labeled.sort_by(|left, right| {
        right.1.estimated_active_keys().cmp(&left.1.estimated_active_keys()).then(left.0.cmp(&right.0))
    });
    let mut table = vec![vec![
        "map".to_string(),
        "files".to_string(),
        "split".to_string(),
        "entries".to_string(),
        "deletions".to_string(),
        "bytes".to_string(),
        "est. keys".to_string(),
        "share".to_string(),
    ]];
    for (label, stats) in labeled {
        table.push(vec![
            label,
            grouped(stats.files),
            grouped(stats.split_files),
            grouped(stats.entries),
            grouped(stats.deletions),
            grouped(stats.bytes),
            grouped_i128(stats.estimated_active_keys()),
            share(stats.estimated_active_keys(), sst_active),
        ]);
    }
    table
}

fn category_label(key: MapKey, show_network: bool) -> String {
    match key {
        MapKey::Map { network_id, map_id } => {
            let name = map_label(map_id)
                .and_then(|label| label.split_once("::").map(|(category, _)| category))
                .unwrap_or("map");
            if show_network { format!("net {network_id} {name}") } else { name.to_string() }
        }
        MapKey::Unattributed => "unattributed".to_string(),
        MapKey::SpanningNotScanned => "spanning".to_string(),
    }
}

fn row_label(key: MapKey, show_network: bool) -> String {
    match key {
        MapKey::Map { network_id, map_id } => {
            let name = match map_label(map_id) {
                Some(label) => label.to_string(),
                None => format!("map {map_id}"),
            };
            if show_network { format!("net {network_id} {name}") } else { name }
        }
        MapKey::Unattributed => "unattributed".to_string(),
        MapKey::SpanningNotScanned => "spanning (not scanned)".to_string(),
    }
}

fn push_table(out: &mut String, rows: &[Vec<String>]) {
    let width = rows.iter().fold(vec![0; rows.first().map(|row| row.len()).unwrap_or(0)], |mut width, row| {
        for (column, cell) in row.iter().enumerate() {
            width[column] = width[column].max(cell.len());
        }
        width
    });
    for row in rows {
        let line = row
            .iter()
            .enumerate()
            .map(|(index, cell)| {
                if index == 0 {
                    format!("{cell:<width$}", width = width[index])
                } else {
                    format!("{cell:>width$}", width = width[index])
                }
            })
            .collect::<Vec<_>>()
            .join("  ");
        push(out, &line);
    }
}

fn share(estimated: i128, sst_active: u64) -> String {
    if sst_active == 0 {
        return "n/a".to_string();
    }
    let tenths = estimated * 1000 / i128::from(sst_active);
    let sign = if tenths < 0 { "-" } else { "" };
    let tenths = tenths.abs();
    format!("{sign}{}.{}%", tenths / 10, tenths % 10)
}

fn format_option(value: Option<u64>) -> String {
    match value {
        Some(value) => grouped(value),
        None => "unavailable".to_string(),
    }
}

fn grouped(value: u64) -> String {
    group_digits(&value.to_string())
}

fn grouped_i128(value: i128) -> String {
    let digits = group_digits(&value.unsigned_abs().to_string());
    if value < 0 { format!("-{digits}") } else { digits }
}

fn group_digits(digits: &str) -> String {
    let mut grouped = String::new();
    for (index, byte) in digits.chars().rev().enumerate() {
        if index > 0 && index % 3 == 0 {
            grouped.push(',');
        }
        grouped.push(byte);
    }
    grouped.chars().rev().collect()
}

fn push(out: &mut String, line: &str) {
    out.push_str(line);
    out.push('\n');
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::MapStats;
    use snarkvm_ledger_store::helpers::rocksdb::BlockMap;
    use std::collections::BTreeMap;

    #[test]
    fn report_names_the_map_and_its_estimate() {
        let mut maps = BTreeMap::new();
        maps.insert(MapKey::Map { network_id: 0, map_id: BlockMap::Header as u16 }, MapStats {
            files: 1,
            split_files: 0,
            entries: 10,
            deletions: 1,
            bytes: 100,
        });
        let attribution = Attribution { maps, spanning_files: 0, scanned_live_keys: 0 };
        let text = render(
            &ReportHeader {
                primary_path: "/ledger".to_string(),
                secondary_path: "/secondary".to_string(),
                network_id: 0,
            },
            &Gauge {
                estimate_num_keys: Some(8),
                active_mem_entries: 0,
                imm_mem_entries: 0,
                active_mem_deletions: 0,
                imm_mem_deletions: 0,
                num_snapshots: 0,
                num_live_versions: 1,
                live_sst_files_size: Some(100),
            },
            &attribution,
        );
        assert!(text.contains("Block::Header"), "{text}");
        assert!(text.contains("recomputed value matches the property."), "{text}");
        assert!(text.contains("100.0%"), "{text}");
    }

    #[test]
    fn thousands_are_grouped() {
        assert_eq!(grouped(0), "0");
        assert_eq!(grouped(1234567), "1,234,567");
        assert_eq!(grouped_i128(-1200), "-1,200");
    }
}
