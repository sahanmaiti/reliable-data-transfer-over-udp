#!/usr/bin/env python3
"""Paired interpretation of the completed 90-trial primary dataset.

Reads results/raw/experiments_raw.json and the Phase 8 summary.csv.
Does not rerun experiments and does not rewrite the raw files.
The 95% interval is the same Student-t interval as plot_results.py,
applied to the five seed-paired differences. df = 4.
"""

from __future__ import annotations

import csv
import hashlib
import json
import sys
from collections import defaultdict
from pathlib import Path
from typing import Any, Callable, Mapping, Optional, Sequence

sys.path.insert(0, str(Path(__file__).resolve().parent))

import plot_results as stats

REPO = Path(__file__).resolve().parents[2]
RAW_JSON = REPO / "results" / "raw" / "experiments_raw.json"
RAW_CSV = REPO / "results" / "raw" / "experiments_raw.csv"
SUMMARY = REPO / "results" / "analysis" / "summary.csv"
OUT = REPO / "results" / "analysis"

EXPECTED_JSON = "d93dc2b18b49775c35587836e042a9daf6dfc844e85427a6be927180c6f0e8be"
EXPECTED_CSV = "ac071d325a07a41e86f70348579a5ee0e7f0e494d9f1ded4809ddb2f05ffa108"

SEEDS = (101, 202, 303, 404, 505)
RATES = (0.0, 0.05, 0.10, 0.15, 0.20, 0.25)
PROTOCOLS = ("StopAndWait", "GoBackN", "SelectiveRepeat")
PAIRS = (
    ("GoBackN", "StopAndWait"),
    ("GoBackN", "SelectiveRepeat"),
    ("StopAndWait", "SelectiveRepeat"),
)

METRIC_GETTERS: dict[str, Callable[[Mapping[str, Any]], float]] = {
    "data_retransmissions": lambda r: float(r["protocol"]["data_retransmissions"]),
    "retransmission_ratio": lambda r: float(r["retransmission_ratio"]),
    "goodput_bytes_sec": lambda r: float(r["goodput_bytes_sec"]),
    "mean_rtt_ms": lambda r: float(r["timing"]["mean_rtt_ms"]),
    "final_rto_ms": lambda r: float(r["timing"]["final_rto_ms"]),
    "timeout_count": lambda r: float(r["timing"]["timeout_count"]),
    "duration_secs": lambda r: float(r["timing"]["duration_secs"]),
    "timeout_retransmissions": lambda r: float(r["protocol"]["timeout_retransmissions"]),
    "premature_retransmissions": lambda r: float(r["protocol"]["premature_retransmissions"]),
}

SUMMARY_TO_COLUMN = {
    "data_retransmissions": "retransmissions",
    "retransmission_ratio": "retransmission_ratio",
    "goodput_bytes_sec": "goodput",
    "mean_rtt_ms": "mean_rtt",
    "final_rto_ms": "final_rto",
    "timeout_count": "timeout",
    "premature_retransmissions": "premature_retransmissions",
    "duration_secs": "duration",
}


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    digest.update(path.read_bytes())
    return digest.hexdigest()


def require_frozen_dataset() -> None:
    json_hash = sha256(RAW_JSON)
    csv_hash = sha256(RAW_CSV)
    if json_hash != EXPECTED_JSON or csv_hash != EXPECTED_CSV:
        raise SystemExit(
            "raw dataset hash mismatch; interpretation stopped\n"
            f"json {json_hash}\ncsv {csv_hash}"
        )


def load_records() -> list[dict[str, Any]]:
    records = json.loads(RAW_JSON.read_text(encoding="utf-8"))
    if len(records) != 90:
        raise SystemExit(f"expected 90 records, found {len(records)}")
    return records


def index_records(records: Sequence[Mapping[str, Any]]) -> dict[tuple[str, str, int], Mapping[str, Any]]:
    indexed = {}
    for record in records:
        key = (
            str(record["meta"]["protocol"]),
            stats.format_rate(float(record["forward_config"]["configured_reorder_rate"])),
            int(record["meta"]["seed"]),
        )
        if key in indexed:
            raise SystemExit(f"duplicate key {key}")
        indexed[key] = record
    return indexed


def paired_differences(
    indexed: Mapping[tuple[str, str, int], Mapping[str, Any]],
    left: str,
    right: str,
    rate: float,
    getter: Callable[[Mapping[str, Any]], float],
) -> list[float]:
    tag = stats.format_rate(rate)
    values = []
    for seed in SEEDS:
        values.append(getter(indexed[(left, tag, seed)]) - getter(indexed[(right, tag, seed)]))
    if len(values) != 5:
        raise SystemExit("paired comparison did not use five seeds")
    return values


def exploratory_p(values: Sequence[float]) -> tuple[str, str]:
    """Two-sided paired t test of mean difference = 0. Uncorrected."""

    error = stats.standard_error(values)
    if error is None or error == 0.0:
        return "", ""
    statistic = stats.mean(values) / error
    probability = min(1.0, 2.0 * stats.student_t_upper_tail(abs(statistic), 4))
    return f"{statistic:.6g}", f"{probability:.6g}"


def comparison_rows(indexed: Mapping[tuple[str, str, int], Mapping[str, Any]]) -> list[dict[str, Any]]:
    rows = []
    for rate in RATES:
        for left, right in PAIRS:
            label = f"{left} - {right}"
            for metric, getter in METRIC_GETTERS.items():
                if metric in ("timeout_retransmissions", "premature_retransmissions"):
                    continue
                values = paired_differences(indexed, left, right, rate, getter)
                low, high = stats.confidence_interval_95(values)
                deviation = stats.sample_std(values)
                error = stats.standard_error(values)
                t_stat, p_value = exploratory_p(values)
                rows.append(
                    {
                        "reorder_rate": stats.format_rate(rate),
                        "comparison": label,
                        "metric": metric,
                        "n": len(values),
                        "mean_difference": stats.mean(values),
                        "std_difference": "" if deviation is None else deviation,
                        "sem_difference": "" if error is None else error,
                        "ci95_low": "" if low is None else low,
                        "ci95_high": "" if high is None else high,
                        "df": 4,
                        "exploratory_t": t_stat,
                        "exploratory_two_sided_p_uncorrected": p_value,
                    }
                )
    return rows


def ratio_rows(indexed: Mapping[tuple[str, str, int], Mapping[str, Any]]) -> list[dict[str, Any]]:
    """Mean of seed-paired ratios. Undefined when any denominator is 0."""

    specs = (
        ("duration_secs", "duration_ratio"),
        ("goodput_bytes_sec", "goodput_ratio"),
        ("data_retransmissions", "retransmission_ratio_of_counts"),
    )
    rows = []
    for rate in RATES:
        tag = stats.format_rate(rate)
        for left, right in PAIRS:
            for metric, name in specs:
                getter = METRIC_GETTERS[metric]
                denominators = [getter(indexed[(right, tag, seed)]) for seed in SEEDS]
                numerators = [getter(indexed[(left, tag, seed)]) for seed in SEEDS]
                if any(value == 0 for value in denominators):
                    rows.append(
                        {
                            "reorder_rate": tag,
                            "comparison": f"{left} / {right}",
                            "metric": name,
                            "n": 5,
                            "mean_ratio": "undefined",
                            "ci95_low": "undefined",
                            "ci95_high": "undefined",
                        }
                    )
                    continue
                ratios = [num / den for num, den in zip(numerators, denominators)]
                low, high = stats.confidence_interval_95(ratios)
                rows.append(
                    {
                        "reorder_rate": tag,
                        "comparison": f"{left} / {right}",
                        "metric": name,
                        "n": 5,
                        "mean_ratio": stats.mean(ratios),
                        "ci95_low": low,
                        "ci95_high": high,
                    }
                )
    return rows


def write_csv(path: Path, rows: Sequence[Mapping[str, Any]]) -> None:
    if not rows:
        raise SystemExit(f"no rows for {path}")
    with path.open("w", newline="", encoding="utf-8") as handle:
        writer = csv.DictWriter(handle, fieldnames=list(rows[0].keys()))
        writer.writeheader()
        writer.writerows(rows)


def summary_lookup() -> dict[tuple[str, str, str], dict[str, str]]:
    table = {}
    with SUMMARY.open(newline="", encoding="utf-8") as handle:
        for row in csv.DictReader(handle):
            table[(row["protocol"], row["configured_reorder_rate"], row["metric"])] = row
    return table


def primary_rows(table: Mapping[tuple[str, str, str], Mapping[str, str]]) -> list[dict[str, Any]]:
    rows = []
    for protocol in PROTOCOLS:
        for rate in RATES:
            tag = stats.format_rate(rate)
            def cell(metric: str) -> Mapping[str, str]:
                return table[(protocol, tag, metric)]

            ret = cell("data_retransmissions")
            ratio = cell("retransmission_ratio")
            goodput = cell("goodput_bytes_sec")
            duration = cell("duration_secs")
            rtt = cell("mean_rtt_ms")
            rto = cell("final_rto_ms")
            timeout = cell("timeout_count")
            premature = cell("premature_retransmissions")
            if any(item["n"] != "5" for item in (ret, ratio, goodput, duration, rtt, rto, timeout, premature)):
                raise SystemExit(f"expected n=5 for {protocol} {tag}")
            rows.append(
                {
                    "protocol": protocol,
                    "reorder_rate": tag,
                    "n": 5,
                    "retransmissions_mean": ret["mean"],
                    "retransmissions_ci95_low": ret["ci95_low"],
                    "retransmissions_ci95_high": ret["ci95_high"],
                    "retransmission_ratio_mean": ratio["mean"],
                    "goodput_mean": goodput["mean"],
                    "goodput_ci95_low": goodput["ci95_low"],
                    "goodput_ci95_high": goodput["ci95_high"],
                    "duration_mean": duration["mean"],
                    "duration_ci95_low": duration["ci95_low"],
                    "duration_ci95_high": duration["ci95_high"],
                    "mean_rtt_mean": rtt["mean"],
                    "final_rto_mean": rto["mean"],
                    "timeout_mean": timeout["mean"],
                    "premature_retransmissions_mean": premature["mean"],
                }
            )
    return rows


def group_rows(
    indexed: Mapping[tuple[str, str, int], Mapping[str, Any]],
    table: Mapping[tuple[str, str, str], Mapping[str, str]],
) -> tuple[list[dict[str, Any]], list[dict[str, Any]]]:
    rto_rows = []
    recovery_rows = []
    for protocol in PROTOCOLS:
        for rate in RATES:
            tag = stats.format_rate(rate)
            members = [indexed[(protocol, tag, seed)] for seed in SEEDS]
            rto_values = [float(item["timing"]["final_rto_ms"]) for item in members]
            rto_rows.append(
                {
                    "protocol": protocol,
                    "reorder_rate": tag,
                    "mean_final_rto_ms": table[(protocol, tag, "final_rto_ms")]["mean"],
                    "min_final_rto_ms": min(rto_values),
                    "max_final_rto_ms": max(rto_values),
                    "mean_timeout_count": table[(protocol, tag, "timeout_count")]["mean"],
                    "mean_rtt_ms": table[(protocol, tag, "mean_rtt_ms")]["mean"],
                    "mean_duration_secs": table[(protocol, tag, "duration_secs")]["mean"],
                    "mean_data_retransmissions": table[(protocol, tag, "data_retransmissions")]["mean"],
                }
            )
            recovery_rows.append(
                {
                    "protocol": protocol,
                    "reorder_rate": tag,
                    "mean_data_retransmissions": table[(protocol, tag, "data_retransmissions")]["mean"],
                    "mean_timeout_count": table[(protocol, tag, "timeout_count")]["mean"],
                    "mean_timeout_retransmissions": table[(protocol, tag, "timeout_retransmissions")]["mean"]
                    if (protocol, tag, "timeout_retransmissions") in table
                    else stats.mean([float(item["protocol"]["timeout_retransmissions"]) for item in members]),
                    "mean_premature_retransmissions": table[(protocol, tag, "premature_retransmissions")]["mean"],
                    "mean_final_rto_ms": table[(protocol, tag, "final_rto_ms")]["mean"],
                    "mean_duration_secs": table[(protocol, tag, "duration_secs")]["mean"],
                }
            )
    return rto_rows, recovery_rows


def paired_figure(path: Path, metric: str, title: str, ylabel: str, rows: Sequence[Mapping[str, Any]]) -> None:
    colors = {
        "GoBackN - StopAndWait": "#6c3483",
        "GoBackN - SelectiveRepeat": "#1a5276",
        "StopAndWait - SelectiveRepeat": "#7b241c",
    }
    series = []
    for _left, _right in PAIRS:
        label = f"{_left} - {_right}"
        chosen = [row for row in rows if row["comparison"] == label and row["metric"] == metric]
        chosen.sort(key=lambda row: float(row["reorder_rate"]))
        series.append(
            {
                "name": label,
                "color": colors[label],
                "xs": [float(row["reorder_rate"]) for row in chosen],
                "ys": [float(row["mean_difference"]) for row in chosen],
                "lows": [float(row["ci95_low"]) for row in chosen],
                "highs": [float(row["ci95_high"]) for row in chosen],
            }
        )
    stats.write_series_svg(path, title, "Configured forward reorder rate", ylabel, series)


def main() -> int:
    require_frozen_dataset()
    if not SUMMARY.is_file():
        raise SystemExit("results/analysis/summary.csv is missing; Phase 8 aggregation was not found")
    indexed = index_records(load_records())
    table = summary_lookup()
    comparisons = comparison_rows(indexed)
    write_csv(OUT / "protocol_comparisons.csv", comparisons)
    write_csv(OUT / "primary_results.csv", primary_rows(table))
    write_csv(OUT / "effect_magnitudes.csv", ratio_rows(indexed))
    rto_rows, recovery_rows = group_rows(indexed, table)
    write_csv(OUT / "rto_behavior.csv", rto_rows)
    write_csv(OUT / "recovery_behavior.csv", recovery_rows)
    paired_figure(
        OUT / "paired_retransmissions_vs_reorder.svg",
        "data_retransmissions",
        "Paired difference in data retransmissions vs configured forward reorder rate",
        "Mean paired difference (packets)",
        comparisons,
    )
    paired_figure(
        OUT / "paired_duration_vs_reorder.svg",
        "duration_secs",
        "Paired difference in virtual duration vs configured forward reorder rate",
        "Mean paired difference (s)",
        comparisons,
    )
    paired_figure(
        OUT / "paired_goodput_vs_reorder.svg",
        "goodput_bytes_sec",
        "Paired difference in goodput vs configured forward reorder rate",
        "Mean paired difference (bytes/s)",
        comparisons,
    )
    print("wrote paired comparisons and three difference figures")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
