#!/usr/bin/env python3
"""Summarize and plot primary-reordering records produced by the Rust driver.

This script does not create measurements. If the raw JSON file is missing or
does not match the ExperimentRecord schema, it exits. It does not read the
old synthetic pilot as a fallback.

Metric means and 95% confidence intervals use only trials whose status is
SUCCESS and whose SHA-256 digests match. Failed, timed-out, and
integrity-failed trials stay in the failure summary and are not rewritten as
zero-valued successes.

The interval is a two-sided Student-t interval:

    mean ± t_{n-1, 0.975} * s / sqrt(n)

s is the sample standard deviation with divisor n-1. For n <= 1 the interval
is omitted.
"""

from __future__ import annotations

import argparse
import csv
import json
import math
import sys
from collections import defaultdict
from pathlib import Path
from typing import Any, Iterable, Mapping, Optional, Sequence

sys.path.insert(0, str(Path(__file__).resolve().parent))

from run_experiments import (  # noqa: E402
    DERIVED_FIELDS,
    REQUIRED_APP_FIELDS,
    REQUIRED_META_FIELDS,
    REQUIRED_SECTIONS,
    format_rate,
    primary_reordering_study,
    window_for,
)

REPO_ROOT = Path(__file__).resolve().parents[2]
DEFAULT_INPUT = REPO_ROOT / "results" / "raw" / "experiments_raw.json"
DEFAULT_OUTPUT = REPO_ROOT / "results" / "analysis"

PROTOCOLS = ("StopAndWait", "GoBackN", "SelectiveRepeat")
PROTOCOL_COLOR = {
    "StopAndWait": "#c0392b",
    "GoBackN": "#d68910",
    "SelectiveRepeat": "#1e8449",
}

# Successful-trial aggregates. Names are the Rust record fields.
PRIMARY_METRICS = (
    ("data_retransmissions", ("protocol", "data_retransmissions"), "packets", "Data retransmissions"),
    ("retransmission_ratio", ("retransmission_ratio",), "ratio", "Retransmission ratio"),
    ("goodput_bytes_sec", ("goodput_bytes_sec",), "bytes/s", "Goodput"),
    ("mean_rtt_ms", ("timing", "mean_rtt_ms"), "ms", "Mean RTT"),
    ("final_rto_ms", ("timing", "final_rto_ms"), "ms", "Final RTO"),
    ("timeout_count", ("timing", "timeout_count"), "events", "Timeout count"),
    ("premature_retransmissions", ("protocol", "premature_retransmissions"), "packets", "Premature retransmissions"),
    ("duration_secs", ("timing", "duration_secs"), "s", "Transfer duration"),
)
DIAGNOSTIC_METRICS = {"timeout_count", "premature_retransmissions", "actual_reorder_fraction"}


class AnalysisError(RuntimeError):
    """The dataset is missing or is not a Phase 4 experiment record list."""


def _betacf(a: float, b: float, x: float) -> float:
    """Continued fraction for the incomplete beta function (Lentz)."""

    max_iter = 200
    eps = 3.0e-14
    tiny = 1.0e-300
    qab = a + b
    qap = a + 1.0
    qam = a - 1.0
    c = 1.0
    d = 1.0 - qab * x / qap
    if abs(d) < tiny:
        d = tiny
    d = 1.0 / d
    h = d
    for m in range(1, max_iter + 1):
        m2 = 2 * m
        aa = m * (b - m) * x / ((qam + m2) * (a + m2))
        d = 1.0 + aa * d
        if abs(d) < tiny:
            d = tiny
        c = 1.0 + aa / c
        if abs(c) < tiny:
            c = tiny
        d = 1.0 / d
        h *= d * c
        aa = -(a + m) * (qab + m) * x / ((a + m2) * (qap + m2))
        d = 1.0 + aa * d
        if abs(d) < tiny:
            d = tiny
        c = 1.0 + aa / c
        if abs(c) < tiny:
            c = tiny
        d = 1.0 / d
        delta = d * c
        h *= delta
        if abs(delta - 1.0) < eps:
            break
    return h


def regularized_incomplete_beta(a: float, b: float, x: float) -> float:
    if x <= 0.0:
        return 0.0
    if x >= 1.0:
        return 1.0
    log_prefix = (
        a * math.log(x)
        + b * math.log(1.0 - x)
        - math.lgamma(a)
        - math.lgamma(b)
        + math.lgamma(a + b)
    )
    if x < (a + 1.0) / (a + b + 2.0):
        return math.exp(log_prefix) * _betacf(a, b, x) / a
    return 1.0 - math.exp(log_prefix) * _betacf(b, a, 1.0 - x) / b


def student_t_upper_tail(t: float, df: int) -> float:
    """P(T > t) for t >= 0 and df >= 1."""

    x = df / (df + t * t)
    return 0.5 * regularized_incomplete_beta(df / 2.0, 0.5, x)


def t_critical_95(df: int) -> float:
    """Two-sided 95% Student-t critical value, t_{df, 0.975}."""

    if df < 1:
        raise ValueError("t critical value requires df >= 1")
    lo, hi = 0.0, 1.0e6
    for _ in range(80):
        mid = 0.5 * (lo + hi)
        if student_t_upper_tail(mid, df) > 0.025:
            lo = mid
        else:
            hi = mid
    return 0.5 * (lo + hi)


def mean(values: Sequence[float]) -> float:
    return sum(values) / len(values)


def sample_std(values: Sequence[float]) -> Optional[float]:
    n = len(values)
    if n < 2:
        return None
    center = mean(values)
    variance = sum((value - center) ** 2 for value in values) / (n - 1)
    return math.sqrt(variance)


def standard_error(values: Sequence[float]) -> Optional[float]:
    deviation = sample_std(values)
    if deviation is None:
        return None
    return deviation / math.sqrt(len(values))


def confidence_interval_95(values: Sequence[float]) -> tuple[Optional[float], Optional[float]]:
    """Return (low, high). Both are None when n <= 1."""

    n = len(values)
    if n <= 1:
        return None, None
    center = mean(values)
    half = t_critical_95(n - 1) * standard_error(values)
    return center - half, center + half


def load_records(path: Path) -> list[dict[str, Any]]:
    if not path.is_file():
        raise AnalysisError(
            f"raw dataset not found: {path}\n"
            "This analysis does not generate measurements and does not use the synthetic pilot."
        )
    payload = json.loads(path.read_text(encoding="utf-8"))
    if isinstance(payload, dict):
        payload = [payload]
    if not isinstance(payload, list):
        raise AnalysisError(f"{path} is not a JSON list of experiment records")
    return payload


def _require_mapping(record: Mapping[str, Any], key: str, index: int) -> Mapping[str, Any]:
    value = record.get(key)
    if not isinstance(value, Mapping):
        raise AnalysisError(f"record {index} is missing object '{key}'")
    return value


def validate_record(record: Mapping[str, Any], index: int) -> None:
    if not isinstance(record, Mapping):
        raise AnalysisError(f"record {index} is not a JSON object")
    if "meta" not in record and "experiment_id" in record:
        raise AnalysisError(
            f"record {index} is a flat object, not a Phase 4 ExperimentRecord. "
            "The old synthetic pilot is not analyzed."
        )
    for section in REQUIRED_SECTIONS:
        _require_mapping(record, section, index)
    for field in DERIVED_FIELDS:
        if field not in record:
            raise AnalysisError(f"record {index} is missing {field}")
    meta = record["meta"]
    app = record["app"]
    for field in REQUIRED_META_FIELDS:
        if field not in meta:
            raise AnalysisError(f"record {index} meta.{field} is missing")
    for field in REQUIRED_APP_FIELDS:
        if field not in app:
            raise AnalysisError(f"record {index} app.{field} is missing")
    for section in ("forward_config", "reverse_config", "timing", "protocol", "channel", "reverse_channel"):
        if not record[section]:
            raise AnalysisError(f"record {index} {section} is empty")
    required_nested = {
        "forward_config": (
            "configured_loss_rate",
            "configured_reorder_rate",
            "configured_duplicate_rate",
            "configured_corrupt_rate",
            "base_delay_ms",
            "jitter_ms",
            "reorder_extra_ms",
        ),
        "rto_config": ("rto_multiplier", "initial_rto_ms", "min_rto_ms", "max_rto_ms"),
        "timing": ("duration_secs", "mean_rtt_ms", "final_rto_ms", "timeout_count"),
        "protocol": ("data_retransmissions", "premature_retransmissions"),
        "channel": ("offered_datagrams", "reordered_datagrams"),
        "meta": ("window_size",),
    }
    for section, fields in required_nested.items():
        for field in fields:
            if field not in record[section]:
                raise AnalysisError(f"record {index} {section}.{field} is missing")


def validate_dataset(records: Sequence[Mapping[str, Any]]) -> None:
    if not records:
        raise AnalysisError("dataset contains no records")
    for index, record in enumerate(records):
        validate_record(record, index)


def _close(left: float, right: float) -> bool:
    return abs(float(left) - float(right)) <= 1e-9


def _rate_in(value: float, choices: Iterable[float]) -> bool:
    return any(_close(value, choice) for choice in choices)


def primary_mismatch(record: Mapping[str, Any]) -> Optional[str]:
    """Return a reason when the record is outside the documented primary study."""

    study = primary_reordering_study()
    meta = record["meta"]
    forward = record["forward_config"]
    rto = record["rto_config"]
    protocol = str(meta["protocol"])
    reasons = []
    if protocol not in study.protocols:
        reasons.append(f"protocol {protocol}")
    reorder = float(forward["configured_reorder_rate"])
    if not _rate_in(reorder, study.reorder_rates):
        reasons.append(f"reorder {reorder}")
    if not _close(forward["configured_loss_rate"], study.loss_rate):
        reasons.append(f"loss {forward['configured_loss_rate']}")
    if int(meta["seed"]) not in study.seeds:
        reasons.append(f"seed {meta['seed']}")
    if not _close(forward["configured_duplicate_rate"], study.duplicate_rate):
        reasons.append("duplicate rate")
    if not _close(forward["configured_corrupt_rate"], study.corrupt_rate):
        reasons.append("corruption rate")
    if int(forward["base_delay_ms"]) != study.base_delay_ms:
        reasons.append("base delay")
    if int(forward["jitter_ms"]) != study.jitter_ms:
        reasons.append("jitter")
    if int(forward["reorder_extra_ms"]) != study.reorder_extra_ms:
        reasons.append("reorder hold")
    if protocol in study.protocols and int(meta["window_size"]) != window_for(protocol, study):
        reasons.append(f"window {meta['window_size']}")
    if not _close(rto["rto_multiplier"], study.rto_multiplier):
        reasons.append("rto multiplier")
    if not _close(rto["min_rto_ms"], study.min_rto_ms):
        reasons.append("min rto")
    if not _close(rto["initial_rto_ms"], study.initial_rto_ms):
        reasons.append("initial rto")
    if not _close(rto["max_rto_ms"], study.max_rto_ms):
        reasons.append("max rto")
    if not reasons:
        return None
    return "; ".join(reasons)


def outcome(record: Mapping[str, Any]) -> str:
    status = str(record["app"]["transfer_status"])
    matched = record["app"]["sha256_match"] is True
    if status == "SUCCESS" and matched:
        return "success"
    if status == "TIMEOUT":
        return "timeout"
    if status == "INTEGRITY_FAIL" or (status == "SUCCESS" and not matched):
        return "integrity"
    return "other"


def configured_reorder(record: Mapping[str, Any]) -> float:
    return float(record["forward_config"]["configured_reorder_rate"])


def actual_reorder_fraction(record: Mapping[str, Any]) -> Optional[float]:
    offered = record["channel"]["offered_datagrams"]
    if offered is None or int(offered) <= 0:
        return None
    return float(record["channel"]["reordered_datagrams"]) / float(offered)


def _metric_value(record: Mapping[str, Any], path: tuple[str, ...]) -> float:
    current: Any = record
    for key in path:
        current = current[key]
    return float(current)


def aggregate(records: Sequence[Mapping[str, Any]]) -> tuple[list[dict[str, Any]], list[dict[str, Any]], list[dict[str, str]]]:
    """Return (metric rows, failure rows, mismatch rows)."""

    mismatches: list[dict[str, str]] = []
    grouped: dict[tuple[str, str], list[Mapping[str, Any]]] = defaultdict(list)
    for record in records:
        reason = primary_mismatch(record)
        if reason is not None:
            mismatches.append(
                {
                    "experiment_id": str(record["meta"]["experiment_id"]),
                    "protocol": str(record["meta"]["protocol"]),
                    "reason": reason,
                }
            )
            continue
        key = (str(record["meta"]["protocol"]), format_rate(configured_reorder(record)))
        grouped[key].append(record)

    metric_rows: list[dict[str, Any]] = []
    failure_rows: list[dict[str, Any]] = []
    metric_specs = list(PRIMARY_METRICS) + [
        ("actual_reorder_fraction", None, "fraction of offered datagrams", "Measured reorder fraction"),
    ]
    for protocol in PROTOCOLS:
        for rate in primary_reordering_study().reorder_rates:
            key = (protocol, format_rate(rate))
            cell = grouped.get(key, [])
            successes = [record for record in cell if outcome(record) == "success"]
            timeouts = sum(1 for record in cell if outcome(record) == "timeout")
            integrity = sum(1 for record in cell if outcome(record) == "integrity")
            other = sum(1 for record in cell if outcome(record) == "other")
            failure_rows.append(
                {
                    "protocol": protocol,
                    "configured_reorder_rate": format_rate(rate),
                    "total_trials": len(cell),
                    "successful_trials": len(successes),
                    "failed_trials": len(cell) - len(successes),
                    "integrity_failures": integrity,
                    "timeout_failures": timeouts,
                    "other_failures": other,
                }
            )
            for name, path, _unit, _title in metric_specs:
                if name == "actual_reorder_fraction":
                    values = [
                        value
                        for value in (actual_reorder_fraction(record) for record in successes)
                        if value is not None
                    ]
                else:
                    values = [_metric_value(record, path) for record in successes]
                low, high = confidence_interval_95(values) if values else (None, None)
                metric_rows.append(
                    {
                        "protocol": protocol,
                        "configured_reorder_rate": format_rate(rate),
                        "metric": name,
                        "n": len(values),
                        "mean": mean(values) if values else "",
                        "std": "" if sample_std(values) is None else sample_std(values),
                        "sem": "" if standard_error(values) is None else standard_error(values),
                        "ci95_low": "" if low is None else low,
                        "ci95_high": "" if high is None else high,
                    }
                )
    return metric_rows, failure_rows, mismatches


def _blank(value: Any) -> str:
    if value is None:
        return ""
    return str(value)


def write_table(path: Path, rows: Sequence[Mapping[str, Any]], fieldnames: Sequence[str]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open("w", newline="", encoding="utf-8") as handle:
        writer = csv.DictWriter(handle, fieldnames=list(fieldnames))
        writer.writeheader()
        for row in rows:
            writer.writerow({key: _blank(row.get(key, "")) for key in fieldnames})


def write_failure_text(path: Path, rows: Sequence[Mapping[str, Any]], mismatches: Sequence[Mapping[str, str]]) -> None:
    lines = [
        "Failure summary for the primary reordering study.",
        "Metric means use only SUCCESS trials whose SHA-256 digests match.",
        "Failed trials are counted here and are not entered as zero successes.",
        "",
    ]
    for row in rows:
        if row["total_trials"] == 0:
            continue
        lines.append(
            f"{row['protocol']} reorder {row['configured_reorder_rate']}: "
            f"total {row['total_trials']}, success {row['successful_trials']}, "
            f"failed {row['failed_trials']}, integrity {row['integrity_failures']}, "
            f"timeout {row['timeout_failures']}, other {row['other_failures']}"
        )
    lines.append("")
    lines.append("Records outside the primary study:")
    if mismatches:
        for row in mismatches:
            lines.append(f"{row['experiment_id']} ({row['protocol']}): {row['reason']}")
    else:
        lines.append("none")
    path.write_text("\n".join(lines) + "\n", encoding="utf-8")


def _svg_escape(text: str) -> str:
    return (
        text.replace("&", "&amp;")
        .replace("<", "&lt;")
        .replace(">", "&gt;")
        .replace('"', "&quot;")
    )


def write_series_svg(
    path: Path,
    title: str,
    xlabel: str,
    ylabel: str,
    series: Sequence[Mapping[str, Any]],
) -> None:
    width, height = 860, 520
    left, right, top, bottom = 90, 40, 70, 70
    plot_w = width - left - right
    plot_h = height - top - bottom
    xs = sorted({x for item in series for x in item["xs"]})
    ys: list[float] = []
    for item in series:
        ys.extend(item["ys"])
        ys.extend(y for y in item["lows"] if y is not None)
        ys.extend(y for y in item["highs"] if y is not None)
    y_min = min(ys) if ys else 0.0
    y_max = max(ys) if ys else 1.0
    if y_max == y_min:
        y_max = y_min + 1.0
    pad = 0.08 * (y_max - y_min)
    y_min -= pad
    y_max += pad

    def sx(value: float) -> float:
        if len(xs) == 1:
            return left + plot_w / 2
        return left + (xs.index(value) / (len(xs) - 1)) * plot_w

    def sy(value: float) -> float:
        return top + (y_max - value) / (y_max - y_min) * plot_h

    parts = [
        f'<svg xmlns="http://www.w3.org/2000/svg" width="{width}" height="{height}" viewBox="0 0 {width} {height}">',
        '<rect width="100%" height="100%" fill="#ffffff"/>',
        f'<text x="{width / 2}" y="32" text-anchor="middle" font-family="sans-serif" font-size="16">{_svg_escape(title)}</text>',
        f'<line x1="{left}" y1="{top + plot_h}" x2="{left + plot_w}" y2="{top + plot_h}" stroke="#222"/>',
        f'<line x1="{left}" y1="{top}" x2="{left}" y2="{top + plot_h}" stroke="#222"/>',
        f'<text x="{left + plot_w / 2}" y="{height - 18}" text-anchor="middle" font-family="sans-serif" font-size="13">{_svg_escape(xlabel)}</text>',
        f'<text x="22" y="{top + plot_h / 2}" text-anchor="middle" font-family="sans-serif" font-size="13" transform="rotate(-90 22 {top + plot_h / 2})">{_svg_escape(ylabel)}</text>',
    ]
    for x_value in xs:
        px = sx(x_value)
        parts.append(
            f'<text x="{px:.1f}" y="{top + plot_h + 22}" text-anchor="middle" font-family="sans-serif" font-size="11">{x_value * 100:.0f}%</text>'
        )
    legend_y = 52
    for index, item in enumerate(series):
        color = item["color"]
        points = []
        for x_value, y_value, low, high in zip(item["xs"], item["ys"], item["lows"], item["highs"]):
            px, py = sx(x_value), sy(y_value)
            points.append(f"{px:.1f},{py:.1f}")
            if low is not None and high is not None:
                parts.append(
                    f'<line x1="{px:.1f}" y1="{sy(low):.1f}" x2="{px:.1f}" y2="{sy(high):.1f}" stroke="{color}" stroke-width="1.5"/>'
                )
            parts.append(f'<circle cx="{px:.1f}" cy="{py:.1f}" r="4" fill="{color}"/>')
        if points:
            parts.append(
                f'<polyline points="{" ".join(points)}" fill="none" stroke="{color}" stroke-width="2"/>'
            )
        lx = left + index * 180
        parts.append(f'<rect x="{lx}" y="{legend_y}" width="14" height="14" fill="{color}"/>')
        parts.append(
            f'<text x="{lx + 20}" y="{legend_y + 12}" font-family="sans-serif" font-size="12">{_svg_escape(item["name"])}</text>'
        )
    parts.append("</svg>")
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text("\n".join(parts) + "\n", encoding="utf-8")


def _series_for_metric(metric_rows: Sequence[Mapping[str, Any]], metric: str) -> list[dict[str, Any]]:
    series = []
    for protocol in PROTOCOLS:
        rows = [
            row
            for row in metric_rows
            if row["protocol"] == protocol and row["metric"] == metric and row["n"]
        ]
        rows.sort(key=lambda row: float(row["configured_reorder_rate"]))
        series.append(
            {
                "name": protocol,
                "color": PROTOCOL_COLOR[protocol],
                "xs": [float(row["configured_reorder_rate"]) for row in rows],
                "ys": [float(row["mean"]) for row in rows],
                "lows": [None if row["ci95_low"] == "" else float(row["ci95_low"]) for row in rows],
                "highs": [None if row["ci95_high"] == "" else float(row["ci95_high"]) for row in rows],
            }
        )
    return series


def write_plots(output_dir: Path, metric_rows: Sequence[Mapping[str, Any]]) -> list[Path]:
    specs = [
        ("data_retransmissions_vs_reorder.svg", "data_retransmissions", "Mean data retransmissions vs configured forward reorder rate", "Mean data retransmissions (packets)", False),
        ("retransmission_ratio_vs_reorder.svg", "retransmission_ratio", "Mean retransmission ratio vs configured forward reorder rate", "Mean retransmission ratio", False),
        ("goodput_vs_reorder.svg", "goodput_bytes_sec", "Mean goodput vs configured forward reorder rate", "Mean goodput (bytes/s)", False),
        ("mean_rtt_vs_reorder.svg", "mean_rtt_ms", "Mean RTT vs configured forward reorder rate", "Mean RTT (ms)", False),
        ("final_rto_vs_reorder.svg", "final_rto_ms", "Mean final RTO vs configured forward reorder rate", "Mean final RTO (ms)", False),
        ("duration_vs_reorder.svg", "duration_secs", "Mean transfer duration vs configured forward reorder rate", "Mean virtual duration (s)", False),
        ("timeout_count_vs_reorder.svg", "timeout_count", "Diagnostic: mean timeout count vs configured forward reorder rate", "Mean timeout count (events)", True),
        ("premature_retransmissions_vs_reorder.svg", "premature_retransmissions", "Diagnostic: mean premature retransmissions vs configured forward reorder rate", "Mean premature retransmissions (packets)", True),
        ("actual_reorder_fraction_vs_configured.svg", "actual_reorder_fraction", "Diagnostic: measured reorder fraction vs configured forward reorder rate", "Mean reordered datagrams / offered datagrams", True),
    ]
    written = []
    for filename, metric, title, ylabel, _diagnostic in specs:
        path = output_dir / filename
        write_series_svg(
            path,
            title,
            "Configured forward reorder rate",
            ylabel,
            _series_for_metric(metric_rows, metric),
        )
        written.append(path)
    return written


def analyze(records: Sequence[Mapping[str, Any]], output_dir: Path) -> dict[str, Any]:
    validate_dataset(records)
    metric_rows, failure_rows, mismatches = aggregate(records)
    output_dir.mkdir(parents=True, exist_ok=True)
    summary_fields = (
        "protocol",
        "configured_reorder_rate",
        "metric",
        "n",
        "mean",
        "std",
        "sem",
        "ci95_low",
        "ci95_high",
    )
    failure_fields = (
        "protocol",
        "configured_reorder_rate",
        "total_trials",
        "successful_trials",
        "failed_trials",
        "integrity_failures",
        "timeout_failures",
        "other_failures",
    )
    write_table(output_dir / "summary.csv", metric_rows, summary_fields)
    write_table(output_dir / "failure_summary.csv", failure_rows, failure_fields)
    write_table(output_dir / "excluded_records.csv", mismatches, ("experiment_id", "protocol", "reason"))
    write_failure_text(output_dir / "failure_summary.txt", failure_rows, mismatches)
    plots = write_plots(output_dir, metric_rows)
    return {
        "summary": output_dir / "summary.csv",
        "failures": output_dir / "failure_summary.csv",
        "excluded": output_dir / "excluded_records.csv",
        "plots": plots,
        "mismatches": len(mismatches),
    }


def main(argv: Optional[Sequence[str]] = None) -> int:
    parser = argparse.ArgumentParser(description="Analyze Rust experiment records. Does not generate data.")
    parser.add_argument("--input", type=Path, default=DEFAULT_INPUT)
    parser.add_argument("--output-dir", type=Path, default=DEFAULT_OUTPUT)
    args = parser.parse_args(argv)
    try:
        records = load_records(args.input)
        result = analyze(records, args.output_dir)
    except (AnalysisError, json.JSONDecodeError, OSError, KeyError, TypeError, ValueError) as exc:
        print(str(exc), file=sys.stderr)
        return 1
    print(f"summary: {result['summary']}")
    print(f"failures: {result['failures']}")
    print(f"excluded: {result['excluded']} ({result['mismatches']} records)")
    for path in result["plots"]:
        print(f"plot: {path}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
